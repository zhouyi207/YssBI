use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use super::dto::{
    DiagnosticBatchDto, DiagnosticDomain, DiagnosticFields, DiagnosticLevel, DiagnosticOrigin,
    DiagnosticRecordDto, DiagnosticSubscriptionDto,
};
use super::sanitizer::{
    sanitize_event, sanitize_fields, sanitize_message, sanitize_source, sanitize_target,
};
use super::worker::{BoundedWorker, EnqueueResult};

pub const RECENT_DIAGNOSTIC_CAPACITY: usize = 5_000;
const DIAGNOSTIC_INGRESS_CAPACITY: usize = 1_024;
const RECORD_OUTPUT_CAPACITY: usize = 1_024;
const SUBSCRIBER_QUEUE_CAPACITY: usize = 8;
const DISPATCH_RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
const DISPATCH_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
pub(crate) const LIVE_BATCH_MAX_RECORDS: usize = 128;
pub(crate) const LIVE_BATCH_INTERVAL: Duration = Duration::from_millis(16);

const DROPPED_EVENT: &str = "diagnostics.records_dropped";
const DROPPED_TARGET: &str = "yssbi::diagnostics";

type BatchSink = Box<dyn Fn(DiagnosticBatchDto) -> bool + Send + 'static>;
pub(crate) type RecordSink = Box<dyn FnMut(&DiagnosticRecordDto) -> bool + Send + 'static>;

#[derive(Debug, Clone)]
pub(crate) struct PendingDiagnostic {
    pub timestamp: String,
    pub level: DiagnosticLevel,
    pub origin: DiagnosticOrigin,
    pub domain: DiagnosticDomain,
    pub target: String,
    pub event: Option<String>,
    pub message: String,
    pub source: Option<String>,
    pub fields: DiagnosticFields,
}

impl PendingDiagnostic {
    fn sanitized(mut self) -> Self {
        self.target = sanitize_target(&self.target);
        self.event = self.event.map(|value| sanitize_event(&value));
        self.message = sanitize_message(&self.message);
        self.source = self.source.map(|value| sanitize_source(&value));
        self.fields = sanitize_fields(self.fields);
        self
    }

    fn records_dropped(dropped_count: u64) -> Self {
        Self {
            timestamp: super::rfc3339_now(),
            level: DiagnosticLevel::Warn,
            origin: DiagnosticOrigin::Rust,
            domain: DiagnosticDomain::System,
            target: DROPPED_TARGET.to_owned(),
            event: Some(DROPPED_EVENT.to_owned()),
            message: "Diagnostic records were dropped because the ingress queue was full".into(),
            source: None,
            fields: BTreeMap::from([("droppedCount".into(), json!(dropped_count))]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("diagnostics dispatcher is unavailable")]
pub struct DiagnosticsUnavailable;

#[derive(Clone)]
pub(crate) struct DiagnosticsHub {
    sender: SyncSender<DispatcherCommand>,
    dropped_records: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
}

pub(crate) struct DiagnosticsDispatcherGuard {
    sender: SyncSender<DispatcherCommand>,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

enum DispatcherCommand {
    Publish(PendingDiagnostic),
    ReportDropped(u64),
    Subscribe {
        sink: BatchSink,
        response: mpsc::Sender<Option<DiagnosticSubscriptionDto>>,
    },
    Unsubscribe {
        subscription_id: String,
        response: mpsc::Sender<()>,
    },
    Shutdown,
}

struct DispatcherState {
    stream_id: String,
    latest_sequence: u64,
    truncated: bool,
    recent: VecDeque<DiagnosticRecordDto>,
    subscriptions: BTreeMap<String, BoundedWorker<Arc<DiagnosticBatchDto>>>,
    live_pending: Vec<DiagnosticRecordDto>,
    live_deadline: Option<Instant>,
    record_outputs: Vec<BoundedWorker<Arc<DiagnosticRecordDto>>>,
    subscriber_queue_capacity: usize,
}

struct DispatcherConfig {
    ingress_capacity: usize,
    record_output_capacity: usize,
    subscriber_queue_capacity: usize,
}

impl DispatcherConfig {
    const fn production() -> Self {
        Self {
            ingress_capacity: DIAGNOSTIC_INGRESS_CAPACITY,
            record_output_capacity: RECORD_OUTPUT_CAPACITY,
            subscriber_queue_capacity: SUBSCRIBER_QUEUE_CAPACITY,
        }
    }
}

impl DiagnosticsHub {
    #[cfg(test)]
    pub(crate) fn start() -> (Self, DiagnosticsDispatcherGuard) {
        Self::start_with_config(DispatcherConfig::production(), Vec::new(), None)
    }

    pub(crate) fn start_with_record_sinks(
        record_sinks: Vec<RecordSink>,
    ) -> (Self, DiagnosticsDispatcherGuard) {
        Self::start_with_config(DispatcherConfig::production(), record_sinks, None)
    }

    fn start_with_config(
        config: DispatcherConfig,
        record_sinks: Vec<RecordSink>,
        startup_gate: Option<Receiver<()>>,
    ) -> (Self, DiagnosticsDispatcherGuard) {
        let (sender, receiver) = mpsc::sync_channel(config.ingress_capacity.max(1));
        let dropped_records = Arc::new(AtomicU64::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_dropped_records = dropped_records.clone();
        let worker_shutdown = shutdown.clone();
        let worker = thread::Builder::new()
            .name("yssbi-diagnostics".into())
            .spawn(move || {
                wait_for_startup(startup_gate, &worker_shutdown);
                run_dispatcher(
                    receiver,
                    worker_dropped_records,
                    worker_shutdown,
                    record_sinks,
                    config,
                );
            })
            .map_err(|error| {
                eprintln!("failed to start diagnostics dispatcher: {error}");
            })
            .ok();
        let hub = Self {
            sender: sender.clone(),
            dropped_records,
            shutdown: shutdown.clone(),
        };
        let guard = DiagnosticsDispatcherGuard {
            sender,
            shutdown,
            worker,
        };
        (hub, guard)
    }

    #[cfg(test)]
    pub(crate) fn start_for_test(
        ingress_capacity: usize,
        subscriber_queue_capacity: usize,
        record_sinks: Vec<RecordSink>,
    ) -> (Self, DiagnosticsDispatcherGuard) {
        Self::start_with_config(
            DispatcherConfig {
                ingress_capacity,
                record_output_capacity: RECORD_OUTPUT_CAPACITY,
                subscriber_queue_capacity,
            },
            record_sinks,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn start_paused_for_test(
        ingress_capacity: usize,
        record_sinks: Vec<RecordSink>,
    ) -> (Self, DiagnosticsDispatcherGuard, mpsc::Sender<()>) {
        let (release, startup_gate) = mpsc::channel();
        let (hub, guard) = Self::start_with_config(
            DispatcherConfig {
                ingress_capacity,
                record_output_capacity: RECORD_OUTPUT_CAPACITY,
                subscriber_queue_capacity: SUBSCRIBER_QUEUE_CAPACITY,
            },
            record_sinks,
            Some(startup_gate),
        );
        (hub, guard, release)
    }

    pub(crate) fn publish(
        &self,
        records: Vec<PendingDiagnostic>,
    ) -> Result<(), DiagnosticsUnavailable> {
        if records.is_empty() {
            return Ok(());
        }
        if self.shutdown.load(Ordering::Acquire) {
            return Err(DiagnosticsUnavailable);
        }

        for record in records {
            self.try_enqueue_dropped_marker()?;
            match self
                .sender
                .try_send(DispatcherCommand::Publish(record.sanitized()))
            {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => self.add_dropped(1),
                Err(TrySendError::Disconnected(_)) => return Err(DiagnosticsUnavailable),
            }
        }
        self.try_enqueue_dropped_marker()
    }

    pub(crate) fn subscribe(
        &self,
        sink: impl Fn(DiagnosticBatchDto) -> bool + Send + 'static,
    ) -> Result<DiagnosticSubscriptionDto, DiagnosticsUnavailable> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(DiagnosticsUnavailable);
        }
        self.try_enqueue_dropped_marker()?;
        let (response, result) = mpsc::channel();
        self.sender
            .try_send(DispatcherCommand::Subscribe {
                sink: Box::new(sink),
                response,
            })
            .map_err(|_| DiagnosticsUnavailable)?;
        result
            .recv_timeout(DISPATCH_RESPONSE_TIMEOUT)
            .map_err(|_| DiagnosticsUnavailable)?
            .ok_or(DiagnosticsUnavailable)
    }

    pub(crate) fn unsubscribe(
        &self,
        subscription_id: String,
    ) -> Result<(), DiagnosticsUnavailable> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(DiagnosticsUnavailable);
        }
        let (response, result) = mpsc::channel();
        self.sender
            .try_send(DispatcherCommand::Unsubscribe {
                subscription_id,
                response,
            })
            .map_err(|_| DiagnosticsUnavailable)?;
        result
            .recv_timeout(DISPATCH_RESPONSE_TIMEOUT)
            .map_err(|_| DiagnosticsUnavailable)
    }

    fn try_enqueue_dropped_marker(&self) -> Result<(), DiagnosticsUnavailable> {
        let dropped_count = self.dropped_records.swap(0, Ordering::AcqRel);
        if dropped_count == 0 {
            return Ok(());
        }
        match self
            .sender
            .try_send(DispatcherCommand::ReportDropped(dropped_count))
        {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.add_dropped(dropped_count);
                Ok(())
            }
            Err(TrySendError::Disconnected(_)) => Err(DiagnosticsUnavailable),
        }
    }

    fn add_dropped(&self, count: u64) {
        if count == 0 {
            return;
        }
        let _ =
            self.dropped_records
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    Some(current.saturating_add(count))
                });
    }
}

impl Drop for DiagnosticsDispatcherGuard {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let _ = self.sender.try_send(DispatcherCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl DispatcherState {
    fn new(record_sinks: Vec<RecordSink>, config: &DispatcherConfig) -> Self {
        let record_outputs = record_sinks
            .into_iter()
            .enumerate()
            .filter_map(|(index, mut sink)| {
                BoundedWorker::spawn(
                    format!("yssbi-diagnostics-output-{index}"),
                    config.record_output_capacity,
                    move |record: Arc<DiagnosticRecordDto>| sink(record.as_ref()),
                )
                .map_err(|error| {
                    eprintln!("failed to start diagnostics output worker: {error}");
                })
                .ok()
            })
            .collect();
        Self {
            stream_id: Uuid::new_v4().to_string(),
            latest_sequence: 0,
            truncated: false,
            recent: VecDeque::with_capacity(RECENT_DIAGNOSTIC_CAPACITY),
            subscriptions: BTreeMap::new(),
            live_pending: Vec::with_capacity(LIVE_BATCH_MAX_RECORDS),
            live_deadline: None,
            record_outputs,
            subscriber_queue_capacity: config.subscriber_queue_capacity,
        }
    }

    fn publish(&mut self, pending: PendingDiagnostic) {
        let Some(sequence) = self.latest_sequence.checked_add(1) else {
            eprintln!("diagnostics sequence exhausted; dropping subsequent records");
            return;
        };
        self.latest_sequence = sequence;
        let record = DiagnosticRecordDto {
            stream_id: self.stream_id.clone(),
            sequence,
            timestamp: pending.timestamp,
            level: pending.level,
            origin: pending.origin,
            domain: pending.domain,
            target: pending.target,
            event: pending.event,
            message: pending.message,
            source: pending.source,
            fields: pending.fields,
        };

        if self.recent.len() == RECENT_DIAGNOSTIC_CAPACITY {
            self.recent.pop_front();
            self.truncated = true;
        }
        self.recent.push_back(record.clone());
        self.fan_out_record(Arc::new(record.clone()));

        if self.subscriptions.is_empty() {
            return;
        }
        if self.live_pending.is_empty() {
            self.live_deadline = Instant::now().checked_add(LIVE_BATCH_INTERVAL);
        }
        self.live_pending.push(record);
        if self.live_pending.len() >= LIVE_BATCH_MAX_RECORDS {
            self.flush_live();
        }
    }

    fn publish_dropped(&mut self, dropped_count: u64) {
        if dropped_count > 0 {
            self.publish(PendingDiagnostic::records_dropped(dropped_count));
        }
    }

    fn fan_out_record(&mut self, record: Arc<DiagnosticRecordDto>) {
        self.record_outputs.retain(|output| {
            if !output.is_active() {
                return false;
            }
            !matches!(output.try_enqueue(record.clone()), EnqueueResult::Closed)
        });
    }

    fn flush_if_due(&mut self, now: Instant) {
        if self.live_deadline.is_some_and(|deadline| now >= deadline) {
            self.flush_live();
        }
    }

    fn flush_live(&mut self) {
        self.live_deadline = None;
        if self.live_pending.is_empty() {
            return;
        }

        let batch = Arc::new(DiagnosticBatchDto {
            stream_id: self.stream_id.clone(),
            entries: std::mem::replace(
                &mut self.live_pending,
                Vec::with_capacity(LIVE_BATCH_MAX_RECORDS),
            ),
        });
        self.subscriptions.retain(|_, subscription| {
            if !subscription.is_active() {
                return false;
            }
            match subscription.try_enqueue(batch.clone()) {
                EnqueueResult::Enqueued => true,
                EnqueueResult::Full | EnqueueResult::Closed => {
                    subscription.deactivate();
                    false
                }
            }
        });
    }

    fn prune_subscriptions(&mut self) {
        self.subscriptions
            .retain(|_, subscription| subscription.is_active());
    }

    fn subscribe(
        &mut self,
        sink: BatchSink,
        response: mpsc::Sender<Option<DiagnosticSubscriptionDto>>,
    ) {
        let subscription_id = Uuid::new_v4().to_string();
        let worker = BoundedWorker::spawn(
            format!("yssbi-diagnostics-subscriber-{subscription_id}"),
            self.subscriber_queue_capacity,
            move |batch: Arc<DiagnosticBatchDto>| sink((*batch).clone()),
        );
        let Ok(worker) = worker else {
            let _ = response.send(None);
            return;
        };
        self.subscriptions.insert(subscription_id.clone(), worker);
        let subscription = DiagnosticSubscriptionDto {
            subscription_id: subscription_id.clone(),
            stream_id: self.stream_id.clone(),
            entries: self.recent.iter().cloned().collect(),
            latest_sequence: self.latest_sequence,
            truncated: self.truncated,
        };
        if response.send(Some(subscription)).is_err() {
            self.subscriptions.remove(&subscription_id);
        }
    }

    fn unsubscribe(&mut self, subscription_id: &str) {
        self.subscriptions.remove(subscription_id);
    }
}

fn wait_for_startup(startup_gate: Option<Receiver<()>>, shutdown: &AtomicBool) {
    let Some(startup_gate) = startup_gate else {
        return;
    };
    while !shutdown.load(Ordering::Acquire) {
        match startup_gate.recv_timeout(Duration::from_millis(10)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn run_dispatcher(
    receiver: Receiver<DispatcherCommand>,
    dropped_records: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    record_sinks: Vec<RecordSink>,
    config: DispatcherConfig,
) {
    let mut state = DispatcherState::new(record_sinks, &config);
    loop {
        if shutdown.load(Ordering::Acquire) {
            drain_for_shutdown(&receiver, &mut state, &dropped_records);
            break;
        }

        state.prune_subscriptions();
        state.flush_if_due(Instant::now());
        let now = Instant::now();
        let poll_deadline = now.checked_add(DISPATCH_IDLE_POLL_INTERVAL).unwrap_or(now);
        let deadline = state.live_deadline.map_or(poll_deadline, |live_deadline| {
            live_deadline.min(poll_deadline)
        });

        match receiver.recv_timeout(deadline.saturating_duration_since(now)) {
            Ok(command) => {
                if process_command(command, &mut state) {
                    shutdown.store(true, Ordering::Release);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let dropped_count = dropped_records.swap(0, Ordering::AcqRel);
                state.publish_dropped(dropped_count);
                state.flush_if_due(Instant::now());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                drain_for_shutdown(&receiver, &mut state, &dropped_records);
                break;
            }
        }
    }
}

fn process_command(command: DispatcherCommand, state: &mut DispatcherState) -> bool {
    match command {
        DispatcherCommand::Publish(record) => state.publish(record),
        DispatcherCommand::ReportDropped(dropped_count) => state.publish_dropped(dropped_count),
        DispatcherCommand::Subscribe { sink, response } => {
            state.flush_live();
            state.subscribe(sink, response);
        }
        DispatcherCommand::Unsubscribe {
            subscription_id,
            response,
        } => {
            state.flush_live();
            state.unsubscribe(&subscription_id);
            let _ = response.send(());
        }
        DispatcherCommand::Shutdown => return true,
    }
    false
}

fn drain_for_shutdown(
    receiver: &Receiver<DispatcherCommand>,
    state: &mut DispatcherState,
    dropped_records: &AtomicU64,
) {
    loop {
        match receiver.try_recv() {
            Ok(DispatcherCommand::Publish(record)) => state.publish(record),
            Ok(DispatcherCommand::ReportDropped(dropped_count)) => {
                state.publish_dropped(dropped_count);
            }
            Ok(DispatcherCommand::Subscribe { response, .. }) => {
                let _ = response.send(None);
            }
            Ok(DispatcherCommand::Unsubscribe { response, .. }) => {
                let _ = response.send(());
            }
            Ok(DispatcherCommand::Shutdown) => {}
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
    }
    state.publish_dropped(dropped_records.swap(0, Ordering::AcqRel));
    state.flush_live();
}
