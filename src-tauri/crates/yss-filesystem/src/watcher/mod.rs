pub mod notify;
use crate::change::FilesystemChange;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

const WATCHER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const WATCHER_REAPER_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WatcherEpoch(u64);

impl WatcherEpoch {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedChange {
    pub epoch: WatcherEpoch,
    pub change: FilesystemChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatcherShutdownControl {
    deadline: Instant,
}

impl WatcherShutdownControl {
    pub fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    #[cfg(test)]
    pub(crate) fn is_expired(self) -> bool {
        Instant::now() >= self.deadline
    }

    pub fn remaining(self) -> Option<Duration> {
        self.deadline.checked_duration_since(Instant::now())
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum FileWatcherStartError {
    #[error("filesystem watcher failed to start")]
    StartFailed,
}

pub trait ChangeSink: Send + Sync {
    fn publish(&self, change: ObservedChange);
}

pub trait FileWatcherSession: Send {
    fn close_admission(self: Box<Self>) -> Box<dyn FileWatcherDrain>;
}

pub enum FileWatcherDrainOutcome {
    Drained,
    WorkerPanicked,
    TimedOut(Box<dyn FileWatcherDrain>),
}

impl fmt::Debug for FileWatcherDrainOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Drained => formatter.write_str("Drained"),
            Self::WorkerPanicked => formatter.write_str("WorkerPanicked"),
            Self::TimedOut(_) => formatter.write_str("TimedOut(<drain>)"),
        }
    }
}

pub trait FileWatcherDrain: Send {
    fn finish(self: Box<Self>, control: WatcherShutdownControl) -> FileWatcherDrainOutcome;
}

pub trait FileWatcherFactory: Send + Sync {
    fn start(
        &self,
        root: &Path,
        epoch: WatcherEpoch,
        sink: Arc<dyn ChangeSink>,
    ) -> Result<Box<dyn FileWatcherSession>, FileWatcherStartError>;
}

#[derive(Error)]
pub enum WatcherError {
    #[error("filesystem watcher failed to start")]
    Start(#[source] FileWatcherStartError),
    #[error("filesystem watcher epoch is exhausted")]
    EpochExhausted,
    #[error("filesystem watcher shutdown timed out")]
    TimedOut(Box<dyn FileWatcherDrain>),
}

impl fmt::Debug for WatcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(error) => formatter.debug_tuple("Start").field(error).finish(),
            Self::EpochExhausted => formatter.write_str("EpochExhausted"),
            Self::TimedOut(_) => formatter.write_str("TimedOut(<drain>)"),
        }
    }
}

struct EpochAdmission {
    epoch: WatcherEpoch,
    state: Mutex<EpochAdmissionState>,
}

#[derive(Default)]
struct EpochAdmissionState {
    closed: bool,
    in_flight: usize,
}

impl EpochAdmission {
    fn new(epoch: WatcherEpoch) -> Self {
        Self {
            epoch,
            state: Mutex::new(EpochAdmissionState::default()),
        }
    }

    fn close_admission(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.closed = true;
    }

    fn admit(self: &Arc<Self>, epoch: WatcherEpoch) -> Option<EpochPermit> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed || epoch != self.epoch {
            return None;
        }
        state.in_flight += 1;
        Some(EpochPermit {
            admission: Arc::clone(self),
        })
    }

    fn leave(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.in_flight = state.in_flight.saturating_sub(1);
    }
}

struct EpochPermit {
    admission: Arc<EpochAdmission>,
}

impl Drop for EpochPermit {
    fn drop(&mut self) {
        self.admission.leave();
    }
}

struct EpochFilteringSink {
    admission: Arc<EpochAdmission>,
    sink: Arc<dyn ChangeSink>,
}

impl ChangeSink for EpochFilteringSink {
    fn publish(&self, change: ObservedChange) {
        let Some(_permit) = self.admission.admit(change.epoch) else {
            return;
        };
        self.sink.publish(change);
    }
}

struct ActiveWatcher {
    epoch: WatcherEpoch,
    admission: Arc<EpochAdmission>,
    session: Box<dyn FileWatcherSession>,
}

enum WatcherLifecycleState {
    Idle,
    Starting {
        epoch: WatcherEpoch,
    },
    Active(ActiveWatcher),
    Closing {
        epoch: WatcherEpoch,
        admission: Arc<EpochAdmission>,
    },
    Draining {
        epoch: WatcherEpoch,
        admission: Arc<EpochAdmission>,
        drain: Arc<DrainCell>,
        finishing: bool,
    },
}

struct WatcherLifecycle {
    state: Mutex<WatcherLifecycleState>,
    changed: Condvar,
    next_epoch: AtomicU64,
}

impl WatcherLifecycle {
    fn new() -> Self {
        Self {
            state: Mutex::new(WatcherLifecycleState::Idle),
            changed: Condvar::new(),
            next_epoch: AtomicU64::new(0),
        }
    }
}

pub struct WatcherState {
    factory: Arc<dyn FileWatcherFactory>,
    lifecycle: WatcherLifecycle,
    shutdown_timeout: Duration,
}

impl WatcherState {
    pub fn new(factory: Arc<dyn FileWatcherFactory>) -> Self {
        Self {
            factory,
            lifecycle: WatcherLifecycle::new(),
            shutdown_timeout: WATCHER_SHUTDOWN_TIMEOUT,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test<S>(factory: Arc<S>) -> Self
    where
        S: FileWatcherFactory + 'static,
    {
        Self::new(factory)
    }

    pub fn watch(
        &self,
        root: impl AsRef<Path>,
        sink: Arc<dyn ChangeSink>,
    ) -> Result<(), WatcherError> {
        self.retire_active(WatcherShutdownControl::after(self.shutdown_timeout))?;

        let (epoch, admission) = self.reserve_start()?;
        let filtered_sink = Arc::new(EpochFilteringSink {
            admission: admission.clone(),
            sink,
        });
        let root = root.as_ref();
        let session = match self.factory.start(root, epoch, filtered_sink) {
            Ok(session) => session,
            Err(error) => {
                self.abort_start(epoch);
                return Err(WatcherError::Start(error));
            }
        };
        self.install_active(epoch, admission, session);
        Ok(())
    }

    pub fn stop(&self) {
        if let Err(error) = self.retire_active(WatcherShutdownControl::new(Instant::now())) {
            tracing::warn!(
                target: "yss_filesystem::watcher",
                log_domain = "system",
                error = %error,
                "Filesystem watcher shutdown remains pending"
            );
        }
    }

    fn reserve_start(&self) -> Result<(WatcherEpoch, Arc<EpochAdmission>), WatcherError> {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        loop {
            if matches!(*state, WatcherLifecycleState::Idle) {
                let next = self
                    .lifecycle
                    .next_epoch
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                        current.checked_add(1)
                    })
                    .map_err(|_| WatcherError::EpochExhausted)?
                    .checked_add(1)
                    .ok_or(WatcherError::EpochExhausted)?;
                let epoch = WatcherEpoch::new(next);
                let admission = Arc::new(EpochAdmission::new(epoch));
                *state = WatcherLifecycleState::Starting { epoch };
                return Ok((epoch, admission));
            }
            state = self
                .lifecycle
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn abort_start(&self, epoch: WatcherEpoch) {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if matches!(*state, WatcherLifecycleState::Starting { epoch: current } if current == epoch)
        {
            *state = WatcherLifecycleState::Idle;
            self.lifecycle.changed.notify_all();
        }
    }

    fn install_active(
        &self,
        epoch: WatcherEpoch,
        admission: Arc<EpochAdmission>,
        session: Box<dyn FileWatcherSession>,
    ) {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if matches!(*state, WatcherLifecycleState::Starting { epoch: current } if current == epoch)
        {
            *state = WatcherLifecycleState::Active(ActiveWatcher {
                epoch,
                admission,
                session,
            });
            self.lifecycle.changed.notify_all();
            return;
        }

        drop(state);
        admission.close_admission();
        let drain = session.close_admission();
        let cell = Arc::new(DrainCell::new(drain));
        spawn_drain_reaper(cell);
    }

    fn retire_active(&self, control: WatcherShutdownControl) -> Result<(), WatcherError> {
        let target = self.begin_retirement();
        let Some(target) = target else {
            return Ok(());
        };

        let (epoch, admission, drain) = match target {
            RetirementTarget::Active(active) => {
                active.admission.close_admission();
                let drain = active.session.close_admission();
                let cell = Arc::new(DrainCell::new(drain));
                let admission = active.admission;
                self.install_draining(active.epoch, admission.clone(), cell.clone());
                (active.epoch, admission, cell)
            }
            RetirementTarget::Draining {
                epoch,
                admission,
                drain,
            } => (epoch, admission, drain),
        };

        let outcome = drain.finish(control);
        match outcome {
            FileWatcherDrainOutcome::Drained => {
                self.finish_draining(epoch, &admission, &drain);
                Ok(())
            }
            FileWatcherDrainOutcome::WorkerPanicked => {
                tracing::error!(
                    target: "yss_filesystem::watcher",
                    log_domain = "system",
                    log_event = "watcherWorkerPanicked",
                    "Filesystem watcher worker panicked while shutting down"
                );
                self.finish_draining(epoch, &admission, &drain);
                Ok(())
            }
            FileWatcherDrainOutcome::TimedOut(timeout_drain) => {
                self.mark_drain_retryable(epoch, &admission, &drain);
                Err(WatcherError::TimedOut(timeout_drain))
            }
        }
    }

    fn begin_retirement(&self) -> Option<RetirementTarget> {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        loop {
            match std::mem::replace(&mut *state, WatcherLifecycleState::Idle) {
                WatcherLifecycleState::Idle => {
                    *state = WatcherLifecycleState::Idle;
                    return None;
                }
                WatcherLifecycleState::Starting { epoch } => {
                    *state = WatcherLifecycleState::Starting { epoch };
                    state = self
                        .lifecycle
                        .changed
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                WatcherLifecycleState::Closing { epoch, admission } => {
                    *state = WatcherLifecycleState::Closing { epoch, admission };
                    state = self
                        .lifecycle
                        .changed
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                WatcherLifecycleState::Active(active) => {
                    *state = WatcherLifecycleState::Closing {
                        epoch: active.epoch,
                        admission: active.admission.clone(),
                    };
                    self.lifecycle.changed.notify_all();
                    return Some(RetirementTarget::Active(active));
                }
                WatcherLifecycleState::Draining {
                    epoch,
                    admission,
                    drain,
                    finishing,
                } if !finishing => {
                    *state = WatcherLifecycleState::Draining {
                        epoch,
                        admission: admission.clone(),
                        drain: drain.clone(),
                        finishing: true,
                    };
                    self.lifecycle.changed.notify_all();
                    return Some(RetirementTarget::Draining {
                        epoch,
                        admission,
                        drain,
                    });
                }
                WatcherLifecycleState::Draining {
                    epoch,
                    admission,
                    drain,
                    finishing,
                } => {
                    *state = WatcherLifecycleState::Draining {
                        epoch,
                        admission,
                        drain,
                        finishing,
                    };
                    state = self
                        .lifecycle
                        .changed
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            }
        }
    }

    fn install_draining(
        &self,
        epoch: WatcherEpoch,
        admission: Arc<EpochAdmission>,
        drain: Arc<DrainCell>,
    ) {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *state = WatcherLifecycleState::Draining {
            epoch,
            admission,
            drain,
            finishing: true,
        };
        self.lifecycle.changed.notify_all();
    }

    fn finish_draining(
        &self,
        epoch: WatcherEpoch,
        admission: &Arc<EpochAdmission>,
        drain: &Arc<DrainCell>,
    ) {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if matches!(
            &*state,
            WatcherLifecycleState::Draining {
                epoch: current,
                admission: current_admission,
                drain: current_drain,
                ..
            } if *current == epoch
                && Arc::ptr_eq(current_admission, admission)
                && Arc::ptr_eq(current_drain, drain)
        ) {
            *state = WatcherLifecycleState::Idle;
            self.lifecycle.changed.notify_all();
        }
    }

    fn mark_drain_retryable(
        &self,
        epoch: WatcherEpoch,
        admission: &Arc<EpochAdmission>,
        drain: &Arc<DrainCell>,
    ) {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if matches!(
            &*state,
            WatcherLifecycleState::Draining {
                epoch: current,
                admission: current_admission,
                drain: current_drain,
                finishing: true,
            } if *current == epoch
                && Arc::ptr_eq(current_admission, admission)
                && Arc::ptr_eq(current_drain, drain)
        ) {
            if let WatcherLifecycleState::Draining { finishing, .. } = &mut *state {
                *finishing = false;
            }
            self.lifecycle.changed.notify_all();
        }
    }
}

enum RetirementTarget {
    Active(ActiveWatcher),
    Draining {
        epoch: WatcherEpoch,
        admission: Arc<EpochAdmission>,
        drain: Arc<DrainCell>,
    },
}

struct DrainCell {
    owner: Mutex<Option<Box<dyn FileWatcherDrain>>>,
    terminal: Mutex<bool>,
}

impl DrainCell {
    fn new(owner: Box<dyn FileWatcherDrain>) -> Self {
        Self {
            owner: Mutex::new(Some(owner)),
            terminal: Mutex::new(false),
        }
    }

    fn handle(self: &Arc<Self>) -> Box<dyn FileWatcherDrain> {
        Box::new(DrainHandle {
            cell: Arc::clone(self),
        })
    }

    fn finish(self: &Arc<Self>, control: WatcherShutdownControl) -> FileWatcherDrainOutcome {
        let owner = self
            .owner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some(owner) = owner else {
            if *self.terminal.lock().unwrap_or_else(PoisonError::into_inner) {
                return FileWatcherDrainOutcome::Drained;
            }
            return FileWatcherDrainOutcome::WorkerPanicked;
        };

        let outcome = owner.finish(control);
        match outcome {
            FileWatcherDrainOutcome::TimedOut(owner) => {
                *self.owner.lock().unwrap_or_else(PoisonError::into_inner) = Some(owner);
                FileWatcherDrainOutcome::TimedOut(self.handle())
            }
            terminal => {
                *self.terminal.lock().unwrap_or_else(PoisonError::into_inner) = true;
                terminal
            }
        }
    }
}

struct DrainHandle {
    cell: Arc<DrainCell>,
}

impl FileWatcherDrain for DrainHandle {
    fn finish(self: Box<Self>, control: WatcherShutdownControl) -> FileWatcherDrainOutcome {
        self.cell.finish(control)
    }
}

fn spawn_drain_reaper(cell: Arc<DrainCell>) {
    let _ = thread::Builder::new()
        .name("yss-filesystem-watcher-reaper".into())
        .spawn(move || {
            while let FileWatcherDrainOutcome::TimedOut(_) =
                cell.finish(WatcherShutdownControl::after(WATCHER_REAPER_TIMEOUT))
            {}
        });
}

impl WatcherShutdownControl {
    fn after(timeout: Duration) -> Self {
        Self::new(Instant::now() + timeout)
    }
}

impl Drop for WatcherState {
    fn drop(&mut self) {
        let state = self
            .lifecycle
            .state
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        let current = std::mem::replace(state, WatcherLifecycleState::Idle);
        let drain = match current {
            WatcherLifecycleState::Active(active) => {
                active.admission.close_admission();
                Some(Arc::new(DrainCell::new(active.session.close_admission())))
            }
            WatcherLifecycleState::Draining { drain, .. } => Some(drain),
            WatcherLifecycleState::Idle
            | WatcherLifecycleState::Starting { .. }
            | WatcherLifecycleState::Closing { .. } => None,
        };
        if let Some(drain) = drain {
            spawn_drain_reaper(drain);
        }
    }
}

#[cfg(test)]
mod tests;
