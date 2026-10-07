use super::admission::{EpochAdmission, EpochFilteringSink};
use super::drain::{DrainCell, spawn_drain_reaper};
use super::{
    ChangeSink, FileWatcherDrainOutcome, FileWatcherFactory, FileWatcherSession, WatcherEpoch,
    WatcherError, WatcherShutdownControl,
};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

const WATCHER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

struct ActiveWatcher {
    epoch: WatcherEpoch,
    admission: Arc<EpochAdmission>,
    session: Box<dyn FileWatcherSession>,
}

#[derive(Default)]
enum WatcherLifecycleState {
    #[default]
    Idle,
    Starting {
        epoch: WatcherEpoch,
    },
    Active(ActiveWatcher),
    Draining {
        epoch: WatcherEpoch,
        drain: Arc<DrainCell>,
    },
}

#[derive(Default)]
struct WatcherLifecycle {
    state: Mutex<WatcherLifecycleState>,
    changed: Condvar,
    next_epoch: AtomicU64,
}

pub struct WatcherState {
    factory: Arc<dyn FileWatcherFactory>,
    lifecycle: WatcherLifecycle,
    pub(super) shutdown_timeout: Duration,
}

impl WatcherState {
    pub fn new(factory: Arc<dyn FileWatcherFactory>) -> Self {
        Self {
            factory,
            lifecycle: WatcherLifecycle::default(),
            shutdown_timeout: WATCHER_SHUTDOWN_TIMEOUT,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test<S: FileWatcherFactory + 'static>(factory: Arc<S>) -> Self {
        Self::new(factory)
    }

    pub fn watch(
        &self,
        root: impl AsRef<Path>,
        sink: Arc<dyn ChangeSink>,
    ) -> Result<(), WatcherError> {
        let control = WatcherShutdownControl::after(self.shutdown_timeout);
        loop {
            self.retire_active(control)?;
            // A competing watch may have reserved this idle slot while the
            // previous drain finished. Reconcile it instead of waiting forever
            // for an active session to become idle on its own.
            let Some(start) = self.reserve_start()? else {
                continue;
            };
            let filtered_sink = Arc::new(EpochFilteringSink {
                admission: start.admission.clone(),
                sink,
            });
            let session = self
                .factory
                .start(root.as_ref(), start.epoch, filtered_sink)
                .map_err(WatcherError::Start)?;
            start.install(session);
            return Ok(());
        }
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

    fn reserve_start(&self) -> Result<Option<WatcherStart<'_>>, WatcherError> {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !matches!(*state, WatcherLifecycleState::Idle) {
            return Ok(None);
        }
        let next = self
            .lifecycle
            .next_epoch
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .map_err(|_| WatcherError::EpochExhausted)?
            + 1;
        let epoch = WatcherEpoch::new(next);
        let admission = Arc::new(EpochAdmission::new(epoch));
        *state = WatcherLifecycleState::Starting { epoch };
        Ok(Some(WatcherStart {
            owner: self,
            epoch,
            admission,
            installed: false,
        }))
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
        admission.close();
        spawn_drain_reaper(Arc::new(DrainCell::new(session)));
    }

    fn retire_active(&self, control: WatcherShutdownControl) -> Result<(), WatcherError> {
        let Some((epoch, drain)) = self.begin_retirement() else {
            return Ok(());
        };
        match drain.finish(control) {
            FileWatcherDrainOutcome::Drained => {}
            FileWatcherDrainOutcome::WorkerPanicked => {
                tracing::error!(
                    target: "yss_filesystem::watcher",
                    log_domain = "system",
                    log_event = "watcherWorkerPanicked",
                    "Filesystem watcher worker panicked while shutting down"
                );
            }
            FileWatcherDrainOutcome::TimedOut(drain) => return Err(WatcherError::TimedOut(drain)),
        }
        self.finish_draining(epoch, &drain);
        Ok(())
    }

    fn begin_retirement(&self) -> Option<(WatcherEpoch, Arc<DrainCell>)> {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        loop {
            match &*state {
                WatcherLifecycleState::Idle => return None,
                WatcherLifecycleState::Starting { .. } => {
                    state = self
                        .lifecycle
                        .changed
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                WatcherLifecycleState::Active(_) => {
                    let WatcherLifecycleState::Active(active) =
                        std::mem::replace(&mut *state, WatcherLifecycleState::Idle)
                    else {
                        unreachable!("active watcher was just checked")
                    };
                    active.admission.close();
                    let drain = Arc::new(DrainCell::new(active.session));
                    *state = WatcherLifecycleState::Draining {
                        epoch: active.epoch,
                        drain: drain.clone(),
                    };
                    return Some((active.epoch, drain));
                }
                WatcherLifecycleState::Draining { epoch, drain } => {
                    return Some((*epoch, drain.clone()));
                }
            }
        }
    }

    fn finish_draining(&self, epoch: WatcherEpoch, drain: &Arc<DrainCell>) {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if matches!(&*state, WatcherLifecycleState::Draining { epoch: current, drain: current_drain } if *current == epoch && Arc::ptr_eq(current_drain, drain))
        {
            *state = WatcherLifecycleState::Idle;
            self.lifecycle.changed.notify_all();
        }
    }
}

struct WatcherStart<'a> {
    owner: &'a WatcherState,
    epoch: WatcherEpoch,
    admission: Arc<EpochAdmission>,
    installed: bool,
}

impl WatcherStart<'_> {
    fn install(mut self, session: Box<dyn FileWatcherSession>) {
        self.owner
            .install_active(self.epoch, self.admission.clone(), session);
        self.installed = true;
    }
}

impl Drop for WatcherStart<'_> {
    fn drop(&mut self) {
        if !self.installed {
            self.admission.close();
            self.owner.abort_start(self.epoch);
        }
    }
}

impl Drop for WatcherState {
    fn drop(&mut self) {
        let state = self
            .lifecycle
            .state
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        let drain = match std::mem::replace(state, WatcherLifecycleState::Idle) {
            WatcherLifecycleState::Active(active) => {
                active.admission.close();
                Some(Arc::new(DrainCell::new(active.session)))
            }
            WatcherLifecycleState::Draining { drain, .. } => Some(drain),
            WatcherLifecycleState::Idle | WatcherLifecycleState::Starting { .. } => None,
        };
        if let Some(drain) = drain {
            spawn_drain_reaper(drain);
        }
    }
}
