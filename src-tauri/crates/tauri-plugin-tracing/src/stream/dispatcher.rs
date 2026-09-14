use crate::store::{LogPage, LogQuery, LogStatistics, LogStore, LogStoreError, MAX_SAFE_SEQUENCE};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use super::dto::{
    LogBatchDto, LogDomain, LogFields, LogLevel, LogOrigin, LogRecordDto, LogSubscriptionDto,
};
use super::rust_projection::project_log_record;
use super::worker::{BoundedWorker, EnqueueResult};
use crate::collector::{
    CapturedLog, OutputHandle, sanitize_event, sanitize_fields, sanitize_message, sanitize_source,
    sanitize_target,
};

pub const RECENT_LOG_CAPACITY: usize = 5_000;
const LOG_INGRESS_CAPACITY: usize = 1_024;
const SUBSCRIBER_QUEUE_CAPACITY: usize = 8;
const DISPATCH_RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
const DISPATCH_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
pub(crate) const LIVE_BATCH_MAX_RECORDS: usize = 128;
pub(crate) const LIVE_BATCH_INTERVAL: Duration = Duration::from_millis(16);

const DROPPED_EVENT: &str = "logs.records_dropped";
const DROPPED_TARGET: &str = "tauri_plugin_tracing";

type BatchSink = Box<dyn Fn(LogBatchDto) -> bool + Send + 'static>;

#[derive(Debug, Clone)]
pub(crate) struct PendingLog {
    pub timestamp: String,
    pub level: LogLevel,
    pub origin: LogOrigin,
    pub domain: LogDomain,
    pub target: String,
    pub event: Option<String>,
    pub message: String,
    pub source: Option<String>,
    pub fields: LogFields,
}

impl PendingLog {
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
            timestamp: super::local_timestamp_now(),
            level: LogLevel::Warn,
            origin: LogOrigin::Rust,
            domain: LogDomain::System,
            target: DROPPED_TARGET.to_owned(),
            event: Some(DROPPED_EVENT.to_owned()),
            message: "Log records were dropped because the ingress queue was full".into(),
            source: None,
            fields: BTreeMap::from([("droppedCount".into(), json!(dropped_count))]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("logs dispatcher is unavailable")]
pub struct LogsUnavailable;

#[derive(Debug, Error)]
pub(crate) enum LogDispatcherStartError {
    #[error("failed to start log dispatcher")]
    Worker(#[from] std::io::Error),
    #[error("failed to initialize log storage")]
    Storage(#[from] LogStoreError),
}

#[derive(Clone)]
pub(crate) struct LogHub {
    sender: SyncSender<DispatcherCommand>,
    dropped_records: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    storage_failed: Arc<AtomicBool>,
}

pub(crate) struct LogDispatcherGuard {
    sender: SyncSender<DispatcherCommand>,
    shutdown: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

enum DispatcherCommand {
    Query {
        query: LogQuery,
        response: mpsc::Sender<Result<LogPage, LogStoreError>>,
    },
    Statistics {
        response: mpsc::Sender<Result<LogStatistics, LogStoreError>>,
    },
    Publish(PendingLog),
    PublishRust {
        record: CapturedLog,
        console: Option<OutputHandle>,
    },
    ReportDropped(u64),
    Subscribe {
        sink: BatchSink,
        response: mpsc::Sender<Option<LogSubscriptionDto>>,
    },
    Unsubscribe {
        subscription_id: String,
        response: mpsc::Sender<()>,
    },
    Shutdown,
}

struct DispatcherState {
    storage: Option<LogStore>,
    storage_failed: Arc<AtomicBool>,
    stream_id: String,
    latest_sequence: u64,
    truncated: bool,
    recent: VecDeque<LogRecordDto>,
    subscriptions: BTreeMap<String, BoundedWorker<Arc<LogBatchDto>>>,
    live_pending: Vec<LogRecordDto>,
    live_deadline: Option<Instant>,
    subscriber_queue_capacity: usize,
}

struct DispatcherConfig {
    ingress_capacity: usize,
    subscriber_queue_capacity: usize,
}

impl DispatcherConfig {
    const fn production() -> Self {
        Self {
            ingress_capacity: LOG_INGRESS_CAPACITY,
            subscriber_queue_capacity: SUBSCRIBER_QUEUE_CAPACITY,
        }
    }
}

impl LogHub {
    #[cfg(test)]
    pub(crate) fn start() -> (Self, LogDispatcherGuard) {
        Self::start_with_config(DispatcherConfig::production(), None, None)
            .expect("start logs dispatcher")
    }

    pub(crate) fn start_production(
        path: PathBuf,
    ) -> Result<(Self, LogDispatcherGuard), LogDispatcherStartError> {
        Self::start_with_config(DispatcherConfig::production(), None, Some(path))
    }

    pub(crate) fn start_memory() -> Result<(Self, LogDispatcherGuard), LogDispatcherStartError> {
        Self::start_with_config(DispatcherConfig::production(), None, None)
    }

    fn start_with_config(
        config: DispatcherConfig,
        startup_gate: Option<Receiver<()>>,
        storage_path: Option<PathBuf>,
    ) -> Result<(Self, LogDispatcherGuard), LogDispatcherStartError> {
        let (sender, receiver) = mpsc::sync_channel(config.ingress_capacity.max(1));
        let dropped_records = Arc::new(AtomicU64::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_dropped_records = dropped_records.clone();
        let worker_shutdown = shutdown.clone();
        let storage_failed = Arc::new(AtomicBool::new(false));
        let worker_storage_failed = storage_failed.clone();
        let (initialized, ready) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("tracing-logs".into())
            .spawn(move || {
                tracing::subscriber::with_default(
                    tracing::subscriber::NoSubscriber::default(),
                    || {
                        let prepared = (|| {
                            let mut state = DispatcherState::new(&config, worker_storage_failed);
                            if let Some(path) = storage_path {
                                let mut storage = LogStore::open(path)?;
                                let snapshot = storage.snapshot(RECENT_LOG_CAPACITY)?;
                                state.stream_id = snapshot.stream_id;
                                state.latest_sequence = snapshot.latest_sequence;
                                state.recent = snapshot.entries.into();
                                state.truncated = snapshot.truncated;
                                state.storage = Some(storage);
                            }
                            Ok::<_, LogStoreError>(state)
                        })();
                        match prepared {
                            Ok(state) => {
                                if initialized.send(Ok(())).is_err() {
                                    return;
                                }
                                wait_for_startup(startup_gate, &worker_shutdown);
                                run_dispatcher(
                                    receiver,
                                    worker_dropped_records,
                                    worker_shutdown,
                                    state,
                                );
                            }
                            Err(error) => {
                                let _ = initialized.send(Err(error));
                            }
                        }
                    },
                );
            })?;
        if let Err(error) = ready.recv().unwrap_or(Err(LogStoreError::Unavailable)) {
            let _ = worker.join();
            return Err(error.into());
        }
        let hub = Self {
            sender: sender.clone(),
            dropped_records,
            shutdown: shutdown.clone(),
            storage_failed,
        };
        let guard = LogDispatcherGuard {
            sender,
            shutdown,
            worker: Mutex::new(Some(worker)),
        };
        Ok((hub, guard))
    }

    #[cfg(test)]
    pub(crate) fn start_for_test(
        ingress_capacity: usize,
        subscriber_queue_capacity: usize,
    ) -> (Self, LogDispatcherGuard) {
        Self::start_with_config(
            DispatcherConfig {
                ingress_capacity,
                subscriber_queue_capacity,
            },
            None,
            None,
        )
        .expect("start logs dispatcher")
    }

    #[cfg(test)]
    pub(crate) fn start_paused_for_test(
        ingress_capacity: usize,
    ) -> (Self, LogDispatcherGuard, mpsc::Sender<()>) {
        let (release, startup_gate) = mpsc::channel();
        let (hub, guard) = Self::start_with_config(
            DispatcherConfig {
                ingress_capacity,
                subscriber_queue_capacity: SUBSCRIBER_QUEUE_CAPACITY,
            },
            Some(startup_gate),
            None,
        )
        .expect("start logs dispatcher");
        (hub, guard, release)
    }

    pub(crate) fn publish(&self, records: Vec<PendingLog>) -> Result<(), LogsUnavailable> {
        if records.is_empty() {
            return Ok(());
        }
        if self.shutdown.load(Ordering::Acquire) || self.storage_failed.load(Ordering::Acquire) {
            return Err(LogsUnavailable);
        }

        for record in records {
            self.try_enqueue_dropped_marker()?;
            match self.sender.try_send(DispatcherCommand::Publish(record)) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => self.add_dropped(1),
                Err(TrySendError::Disconnected(_)) => return Err(LogsUnavailable),
            }
        }
        self.try_enqueue_dropped_marker()
    }

    pub(crate) fn publish_rust(
        &self,
        record: CapturedLog,
        console: Option<OutputHandle>,
    ) -> Result<(), LogsUnavailable> {
        // Console delivery continues after storage fails. Raw captures stay
        // internal and are only sanitized if the bounded queue accepts them.
        if self.shutdown.load(Ordering::Acquire) {
            return Err(LogsUnavailable);
        }
        self.try_enqueue_dropped_marker()?;
        match self
            .sender
            .try_send(DispatcherCommand::PublishRust { record, console })
        {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => self.add_dropped(1),
            Err(TrySendError::Disconnected(_)) => return Err(LogsUnavailable),
        }
        self.try_enqueue_dropped_marker()
    }

    pub(crate) fn subscribe(
        &self,
        sink: impl Fn(LogBatchDto) -> bool + Send + 'static,
    ) -> Result<LogSubscriptionDto, LogsUnavailable> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(LogsUnavailable);
        }
        self.try_enqueue_dropped_marker()?;
        let (response, result) = mpsc::channel();
        self.sender
            .try_send(DispatcherCommand::Subscribe {
                sink: Box::new(sink),
                response,
            })
            .map_err(|_| LogsUnavailable)?;
        result
            .recv_timeout(DISPATCH_RESPONSE_TIMEOUT)
            .map_err(|_| LogsUnavailable)?
            .ok_or(LogsUnavailable)
    }

    pub(crate) fn unsubscribe(&self, subscription_id: String) -> Result<(), LogsUnavailable> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(LogsUnavailable);
        }
        let (response, result) = mpsc::channel();
        self.sender
            .try_send(DispatcherCommand::Unsubscribe {
                subscription_id,
                response,
            })
            .map_err(|_| LogsUnavailable)?;
        result
            .recv_timeout(DISPATCH_RESPONSE_TIMEOUT)
            .map_err(|_| LogsUnavailable)
    }

    pub(crate) fn query(&self, query: LogQuery) -> Result<LogPage, LogStoreError> {
        let (response, result) = mpsc::channel();
        self.sender
            .try_send(DispatcherCommand::Query { query, response })
            .map_err(|_| LogStoreError::Unavailable)?;
        result
            .recv_timeout(DISPATCH_RESPONSE_TIMEOUT)
            .map_err(|_| LogStoreError::Unavailable)?
    }

    pub(crate) fn statistics(&self) -> Result<LogStatistics, LogStoreError> {
        let (response, result) = mpsc::channel();
        self.sender
            .try_send(DispatcherCommand::Statistics { response })
            .map_err(|_| LogStoreError::Unavailable)?;
        result
            .recv_timeout(DISPATCH_RESPONSE_TIMEOUT)
            .map_err(|_| LogStoreError::Unavailable)?
    }

    fn try_enqueue_dropped_marker(&self) -> Result<(), LogsUnavailable> {
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
            Err(TrySendError::Disconnected(_)) => Err(LogsUnavailable),
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

impl LogDispatcherGuard {
    pub(crate) fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        let _ = self.sender.try_send(DispatcherCommand::Shutdown);
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            let _ = worker.join();
        }
    }
}

impl Drop for LogDispatcherGuard {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl DispatcherState {
    fn new(config: &DispatcherConfig, storage_failed: Arc<AtomicBool>) -> Self {
        Self {
            storage: None,
            storage_failed,
            stream_id: Uuid::new_v4().to_string(),
            latest_sequence: 0,
            truncated: false,
            recent: VecDeque::with_capacity(RECENT_LOG_CAPACITY),
            subscriptions: BTreeMap::new(),
            live_pending: Vec::with_capacity(LIVE_BATCH_MAX_RECORDS),
            live_deadline: None,
            subscriber_queue_capacity: config.subscriber_queue_capacity,
        }
    }

    fn publish(&mut self, pending: PendingLog) {
        self.publish_sanitized(pending.sanitized());
    }

    fn publish_rust(&mut self, captured: CapturedLog, console: Option<OutputHandle>) {
        let record = captured.into_sanitized_record();
        if let Some(console) = console {
            // The console worker owns its copy; no fields are cloned or
            // sanitized on the calculation thread.
            console.try_enqueue(Arc::new(record.clone()));
        }
        self.publish_sanitized(project_log_record(record));
    }

    fn publish_sanitized(&mut self, pending: PendingLog) {
        if self.storage_failed.load(Ordering::Acquire) {
            return;
        }
        let Some(sequence) = self.latest_sequence.checked_add(1) else {
            self.truncated = true;
            return;
        };
        if sequence > MAX_SAFE_SEQUENCE {
            self.truncated = true;
            return;
        }
        self.latest_sequence = sequence;
        let record = LogRecordDto {
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
            self.publish(PendingLog::records_dropped(dropped_count));
        }
    }

    fn flush_if_due(&mut self, now: Instant) {
        if self.live_deadline.is_some_and(|deadline| now >= deadline) {
            self.flush_live();
        }
    }

    fn flush_live(&mut self) -> bool {
        self.live_deadline = None;
        if self.storage_failed.load(Ordering::Acquire) {
            return false;
        }
        if self.live_pending.is_empty() {
            return true;
        }
        let entries = std::mem::replace(
            &mut self.live_pending,
            Vec::with_capacity(LIVE_BATCH_MAX_RECORDS),
        );
        if self
            .storage
            .as_mut()
            .is_some_and(|storage| storage.append(&entries).is_err())
        {
            self.storage_failed.store(true, Ordering::Release);
            self.truncated = true;
            let failure = Arc::new(LogBatchDto {
                stream_id: self.stream_id.clone(),
                entries: Vec::new(),
                failure: Some("storage_unavailable".into()),
            });
            for subscription in self.subscriptions.values() {
                let _ = subscription.try_enqueue(failure.clone());
            }
            return false;
        }
        for record in &entries {
            if self.recent.len() == RECENT_LOG_CAPACITY {
                self.recent.pop_front();
                self.truncated = true;
            }
            self.recent.push_back(record.clone());
        }
        let batch = Arc::new(LogBatchDto {
            stream_id: self.stream_id.clone(),
            entries,
            failure: None,
        });
        self.subscriptions.retain(|_, subscription| {
            subscription.try_enqueue(batch.clone()) == EnqueueResult::Enqueued
        });
        true
    }

    fn prune_subscriptions(&mut self) {
        self.subscriptions
            .retain(|_, subscription| subscription.is_active());
    }

    fn subscribe(&mut self, sink: BatchSink, response: mpsc::Sender<Option<LogSubscriptionDto>>) {
        let subscription_id = Uuid::new_v4().to_string();
        let worker = BoundedWorker::spawn(
            format!("tracing-logs-subscriber-{subscription_id}"),
            self.subscriber_queue_capacity,
            move |batch: Arc<LogBatchDto>| sink((*batch).clone()),
        );
        let Ok(worker) = worker else {
            let _ = response.send(None);
            return;
        };
        self.subscriptions.insert(subscription_id.clone(), worker);
        let subscription = LogSubscriptionDto {
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
    mut state: DispatcherState,
) {
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
        DispatcherCommand::Query { query, response } => {
            let result = if state.flush_live() {
                state
                    .storage
                    .as_mut()
                    .ok_or(LogStoreError::Unavailable)
                    .and_then(|store| store.query(query))
            } else {
                Err(LogStoreError::Unavailable)
            };
            let _ = response.send(result);
        }
        DispatcherCommand::Statistics { response } => {
            let result = if state.flush_live() {
                state
                    .storage
                    .as_mut()
                    .ok_or(LogStoreError::Unavailable)
                    .and_then(LogStore::statistics)
            } else {
                Err(LogStoreError::Unavailable)
            };
            let _ = response.send(result);
        }
        DispatcherCommand::Publish(record) => state.publish(record),
        DispatcherCommand::PublishRust { record, console } => state.publish_rust(record, console),
        DispatcherCommand::ReportDropped(dropped_count) => state.publish_dropped(dropped_count),
        DispatcherCommand::Subscribe { sink, response } => {
            if state.flush_live() {
                state.subscribe(sink, response);
            } else {
                let _ = response.send(None);
            }
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
            Ok(DispatcherCommand::Query { response, .. }) => {
                let _ = response.send(Err(LogStoreError::Unavailable));
            }
            Ok(DispatcherCommand::Statistics { response }) => {
                let _ = response.send(Err(LogStoreError::Unavailable));
            }
            Ok(DispatcherCommand::Publish(record)) => state.publish(record),
            Ok(DispatcherCommand::PublishRust { record, console }) => {
                state.publish_rust(record, console);
            }
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
