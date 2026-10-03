//! Session work admission and drain, independent of kernel scheduling.
use crate::error::{ExecutePreparedError, ExecutionAdmissionError};
use crate::run_registry::{RunId, RunRegistry};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Instant;

#[derive(Default)]
struct RuntimeAdmission {
    closed: bool,
    active: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionDrainControl {
    deadline: Instant,
}

impl ExecutionDrainControl {
    pub const fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    pub(crate) const fn deadline(self) -> Instant {
        self.deadline
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExecutionOutstandingWork {
    pub(super) active: usize,
}

impl ExecutionOutstandingWork {
    const fn is_empty(self) -> bool {
        self.active == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionDrainOutcome {
    Drained {
        outstanding: ExecutionOutstandingWork,
    },
    TimedOut {
        outstanding: ExecutionOutstandingWork,
    },
}

#[must_use = "an execution work lease releases session admission when dropped"]
pub struct ExecutionWorkLease {
    admission: Arc<SessionAdmission>,
}

#[derive(Default)]
pub(super) struct SessionAdmission {
    state: Mutex<RuntimeAdmission>,
    changed: Condvar,
}

impl SessionAdmission {
    pub(super) fn register_run(
        &self,
        runs: &RunRegistry,
        cancellation: Arc<AtomicBool>,
    ) -> Result<RunId, ExecutePreparedError> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed {
            return Err(ExecutePreparedError::Admission(
                ExecutionAdmissionError::Closed,
            ));
        }
        // Keep the admission lock until registration, so close/cancel cannot miss a new run.
        runs.admit_next(Some(cancellation))
            .map_err(ExecutePreparedError::RunRegistry)
    }

    pub(super) fn cancel_and_close(&self, runs: &RunRegistry) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.closed = true;
        runs.cancel_all();
    }

    pub(super) fn close(&self) {
        let state = &self.state;
        state.lock().unwrap_or_else(PoisonError::into_inner).closed = true;
    }

    pub(super) fn admit(self: &Arc<Self>) -> Result<ExecutionWorkLease, ExecutionAdmissionError> {
        let state = &self.state;
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed {
            return Err(ExecutionAdmissionError::Closed);
        }
        state.active += 1;
        drop(state);
        Ok(ExecutionWorkLease {
            admission: Arc::clone(self),
        })
    }

    pub(super) fn drain(&self, control: &ExecutionDrainControl) -> ExecutionDrainOutcome {
        let (state, changed) = (&self.state, &self.changed);
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            let outstanding = ExecutionOutstandingWork {
                active: state.active,
            };
            if outstanding.is_empty() {
                return ExecutionDrainOutcome::Drained { outstanding };
            }

            let Some(remaining) = control.deadline().checked_duration_since(Instant::now()) else {
                return ExecutionDrainOutcome::TimedOut { outstanding };
            };
            let (next_state, _) = changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|error| error.into_inner());
            state = next_state;
        }
    }
}

impl Drop for ExecutionWorkLease {
    fn drop(&mut self) {
        let (state, changed) = (&self.admission.state, &self.admission.changed);
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        debug_assert!(state.active > 0);
        state.active = state.active.saturating_sub(1);
        drop(state);
        changed.notify_all();
    }
}
