use crate::project::{FileChange, project_root_from_path};
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

pub const PROJECT_WATCHER_QUIET_PERIOD: Duration = Duration::from_millis(250);
const PROJECT_WATCHER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const PROJECT_WATCHER_REAPER_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectWatcherEpoch(u64);

impl ProjectWatcherEpoch {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedProjectFileChange {
    pub epoch: ProjectWatcherEpoch,
    pub change: FileChange,
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

    pub(crate) fn remaining(self) -> Option<Duration> {
        self.deadline.checked_duration_since(Instant::now())
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum FileWatcherStartError {
    #[error("project file watcher failed to start")]
    StartFailed,
}

pub trait ProjectFileChangeSink: Send + Sync {
    fn publish(&self, change: ObservedProjectFileChange);
}

pub trait ProjectFileWatcherSession: Send {
    fn close_admission(self: Box<Self>) -> Box<dyn ProjectFileWatcherDrain>;
}

pub enum ProjectFileWatcherDrainOutcome {
    Drained,
    DeliveryFailed,
    WorkerPanicked,
    TimedOut(Box<dyn ProjectFileWatcherDrain>),
}

impl fmt::Debug for ProjectFileWatcherDrainOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Drained => formatter.write_str("Drained"),
            Self::DeliveryFailed => formatter.write_str("DeliveryFailed"),
            Self::WorkerPanicked => formatter.write_str("WorkerPanicked"),
            Self::TimedOut(_) => formatter.write_str("TimedOut(<drain>)"),
        }
    }
}

pub trait ProjectFileWatcherDrain: Send {
    fn finish(self: Box<Self>, control: WatcherShutdownControl) -> ProjectFileWatcherDrainOutcome;
}

pub trait ProjectFileWatcherFactory: Send + Sync {
    fn start(
        &self,
        project_root: &Path,
        epoch: ProjectWatcherEpoch,
        sink: Arc<dyn ProjectFileChangeSink>,
    ) -> Result<Box<dyn ProjectFileWatcherSession>, FileWatcherStartError>;
}

#[derive(Error)]
pub enum ProjectWatcherError {
    #[error("project file watcher failed to start")]
    Start(#[source] FileWatcherStartError),
    #[error("project watcher epoch is exhausted")]
    EpochExhausted,
    #[error("project watcher shutdown timed out")]
    TimedOut(Box<dyn ProjectFileWatcherDrain>),
}

impl fmt::Debug for ProjectWatcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(error) => formatter.debug_tuple("Start").field(error).finish(),
            Self::EpochExhausted => formatter.write_str("EpochExhausted"),
            Self::TimedOut(_) => formatter.write_str("TimedOut(<drain>)"),
        }
    }
}

struct EpochAdmission {
    epoch: ProjectWatcherEpoch,
    state: Mutex<EpochAdmissionState>,
}

#[derive(Default)]
struct EpochAdmissionState {
    closed: bool,
    in_flight: usize,
}

impl EpochAdmission {
    fn new(epoch: ProjectWatcherEpoch) -> Self {
        Self {
            epoch,
            state: Mutex::new(EpochAdmissionState::default()),
        }
    }

    fn close_admission(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.closed = true;
    }

    fn admit(self: &Arc<Self>, epoch: ProjectWatcherEpoch) -> Option<EpochPermit> {
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
    sink: Arc<dyn ProjectFileChangeSink>,
}

impl ProjectFileChangeSink for EpochFilteringSink {
    fn publish(&self, change: ObservedProjectFileChange) {
        let Some(_permit) = self.admission.admit(change.epoch) else {
            return;
        };
        self.sink.publish(change);
    }
}

struct ActiveWatcher {
    epoch: ProjectWatcherEpoch,
    admission: Arc<EpochAdmission>,
    session: Box<dyn ProjectFileWatcherSession>,
}

enum WatcherLifecycleState {
    Idle,
    Starting {
        epoch: ProjectWatcherEpoch,
    },
    Active(ActiveWatcher),
    Closing {
        epoch: ProjectWatcherEpoch,
        admission: Arc<EpochAdmission>,
    },
    Draining {
        epoch: ProjectWatcherEpoch,
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

pub struct ProjectWatcherState {
    factory: Arc<dyn ProjectFileWatcherFactory>,
    lifecycle: WatcherLifecycle,
    shutdown_timeout: Duration,
}

impl ProjectWatcherState {
    pub fn new(factory: Arc<dyn ProjectFileWatcherFactory>) -> Self {
        Self {
            factory,
            lifecycle: WatcherLifecycle::new(),
            shutdown_timeout: PROJECT_WATCHER_SHUTDOWN_TIMEOUT,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test<S>(factory: Arc<S>) -> Self
    where
        S: ProjectFileWatcherFactory + 'static,
    {
        Self::new(factory)
    }

    pub fn watch_project(
        &self,
        metadata_path: &str,
        sink: Arc<dyn ProjectFileChangeSink>,
    ) -> Result<(), ProjectWatcherError> {
        self.retire_active(WatcherShutdownControl::after(self.shutdown_timeout))?;

        let (epoch, admission) = self.reserve_start()?;
        let filtered_sink = Arc::new(EpochFilteringSink {
            admission: admission.clone(),
            sink,
        });
        let root = project_root_from_path(metadata_path);
        let session = match self.factory.start(root.as_path(), epoch, filtered_sink) {
            Ok(session) => session,
            Err(error) => {
                self.abort_start(epoch);
                return Err(ProjectWatcherError::Start(error));
            }
        };
        self.install_active(epoch, admission, session);
        Ok(())
    }

    pub fn stop(&self) {
        if let Err(error) = self.retire_active(WatcherShutdownControl::new(Instant::now())) {
            tracing::warn!(
                target: "yssbi::application::project_watcher",
                diagnostic_domain = "system",
                error = %error,
                "Project watcher shutdown remains pending"
            );
        }
    }

    fn reserve_start(
        &self,
    ) -> Result<(ProjectWatcherEpoch, Arc<EpochAdmission>), ProjectWatcherError> {
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
                    .map_err(|_| ProjectWatcherError::EpochExhausted)?
                    .checked_add(1)
                    .ok_or(ProjectWatcherError::EpochExhausted)?;
                let epoch = ProjectWatcherEpoch::new(next);
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

    fn abort_start(&self, epoch: ProjectWatcherEpoch) {
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
        epoch: ProjectWatcherEpoch,
        admission: Arc<EpochAdmission>,
        session: Box<dyn ProjectFileWatcherSession>,
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

    fn retire_active(&self, control: WatcherShutdownControl) -> Result<(), ProjectWatcherError> {
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
            ProjectFileWatcherDrainOutcome::Drained => {
                self.finish_draining(epoch, &admission, &drain);
                Ok(())
            }
            ProjectFileWatcherDrainOutcome::DeliveryFailed => {
                tracing::warn!(
                    target: "yssbi::application::project_watcher",
                    diagnostic_domain = "system",
                    diagnostic_event = "projectWatcherDeliveryFailed",
                    "Project watcher delivery failed while shutting down"
                );
                self.finish_draining(epoch, &admission, &drain);
                Ok(())
            }
            ProjectFileWatcherDrainOutcome::WorkerPanicked => {
                tracing::error!(
                    target: "yssbi::application::project_watcher",
                    diagnostic_domain = "system",
                    diagnostic_event = "projectWatcherWorkerPanicked",
                    "Project watcher worker panicked while shutting down"
                );
                self.finish_draining(epoch, &admission, &drain);
                Ok(())
            }
            ProjectFileWatcherDrainOutcome::TimedOut(timeout_drain) => {
                self.mark_drain_retryable(epoch, &admission, &drain);
                Err(ProjectWatcherError::TimedOut(timeout_drain))
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
        epoch: ProjectWatcherEpoch,
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
        epoch: ProjectWatcherEpoch,
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
        epoch: ProjectWatcherEpoch,
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
        epoch: ProjectWatcherEpoch,
        admission: Arc<EpochAdmission>,
        drain: Arc<DrainCell>,
    },
}

struct DrainCell {
    owner: Mutex<Option<Box<dyn ProjectFileWatcherDrain>>>,
    terminal: Mutex<bool>,
}

impl DrainCell {
    fn new(owner: Box<dyn ProjectFileWatcherDrain>) -> Self {
        Self {
            owner: Mutex::new(Some(owner)),
            terminal: Mutex::new(false),
        }
    }

    fn handle(self: &Arc<Self>) -> Box<dyn ProjectFileWatcherDrain> {
        Box::new(DrainHandle {
            cell: Arc::clone(self),
        })
    }

    fn finish(self: &Arc<Self>, control: WatcherShutdownControl) -> ProjectFileWatcherDrainOutcome {
        let owner = self
            .owner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some(owner) = owner else {
            if *self.terminal.lock().unwrap_or_else(PoisonError::into_inner) {
                return ProjectFileWatcherDrainOutcome::Drained;
            }
            return ProjectFileWatcherDrainOutcome::WorkerPanicked;
        };

        let outcome = owner.finish(control);
        match outcome {
            ProjectFileWatcherDrainOutcome::TimedOut(owner) => {
                *self.owner.lock().unwrap_or_else(PoisonError::into_inner) = Some(owner);
                ProjectFileWatcherDrainOutcome::TimedOut(self.handle())
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

impl ProjectFileWatcherDrain for DrainHandle {
    fn finish(self: Box<Self>, control: WatcherShutdownControl) -> ProjectFileWatcherDrainOutcome {
        self.cell.finish(control)
    }
}

fn spawn_drain_reaper(cell: Arc<DrainCell>) {
    let _ = thread::Builder::new()
        .name("yssbi-project-watcher-reaper".into())
        .spawn(move || {
            loop {
                match cell.finish(WatcherShutdownControl::after(
                    PROJECT_WATCHER_REAPER_TIMEOUT,
                )) {
                    ProjectFileWatcherDrainOutcome::TimedOut(_) => {}
                    ProjectFileWatcherDrainOutcome::Drained
                    | ProjectFileWatcherDrainOutcome::DeliveryFailed
                    | ProjectFileWatcherDrainOutcome::WorkerPanicked => break,
                }
            }
        });
}

impl WatcherShutdownControl {
    fn after(timeout: Duration) -> Self {
        Self::new(Instant::now() + timeout)
    }
}

impl Drop for ProjectWatcherState {
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
#[path = "project_watcher/tests.rs"]
mod tests;
