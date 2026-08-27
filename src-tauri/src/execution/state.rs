use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Instant;

use thiserror::Error;

use crate::execution::identity::{ExecutionSessionId, RuntimeGeneration};
use crate::execution::result_store::ResultStore;
use crate::execution::run_registry::RunRegistry;

#[derive(Default)]
struct RuntimeAdmission {
    closed: bool,
    active: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub(crate) enum ExecutionAdmissionError {
    #[error("execution session admission is closed")]
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExecutionDrainControl {
    deadline: Instant,
}

impl ExecutionDrainControl {
    pub(crate) const fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    pub(crate) const fn deadline(self) -> Instant {
        self.deadline
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ExecutionOutstandingWork {
    pub(crate) active: usize,
}

impl ExecutionOutstandingWork {
    const fn is_empty(self) -> bool {
        self.active == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecutionDrainOutcome {
    Drained {
        outstanding: ExecutionOutstandingWork,
    },
    TimedOut {
        outstanding: ExecutionOutstandingWork,
    },
}

#[must_use = "an execution work lease releases session admission when dropped"]
pub(crate) struct ExecutionWorkLease {
    admission: Arc<(Mutex<RuntimeAdmission>, Condvar)>,
}

/// Session-local execution state. It is intentionally not installed by the
/// current composition root until the atomic runtime cutover.
pub struct ExecutionRuntimeState {
    session_id: ExecutionSessionId,
    generation: RuntimeGeneration,
    admission: Arc<(Mutex<RuntimeAdmission>, Condvar)>,
    results: ResultStore,
    runs: RunRegistry,
}

impl ExecutionRuntimeState {
    pub fn new(session_id: ExecutionSessionId, generation: RuntimeGeneration) -> Self {
        Self {
            session_id,
            generation,
            admission: Arc::new((Mutex::new(RuntimeAdmission::default()), Condvar::new())),
            results: ResultStore::new(),
            runs: RunRegistry::new(),
        }
    }

    pub fn session_id(&self) -> ExecutionSessionId {
        self.session_id
    }

    pub fn generation(&self) -> RuntimeGeneration {
        self.generation
    }

    pub(crate) fn close_admission(&self) {
        let (state, _) = &*self.admission;
        state.lock().unwrap_or_else(PoisonError::into_inner).closed = true;
    }

    pub fn is_admission_closed(&self) -> bool {
        let (state, _) = &*self.admission;
        state.lock().unwrap_or_else(PoisonError::into_inner).closed
    }

    pub(crate) fn results(&self) -> &ResultStore {
        &self.results
    }

    pub(crate) fn runs(&self) -> &RunRegistry {
        &self.runs
    }

    pub(crate) fn admit(&self) -> Result<ExecutionWorkLease, ExecutionAdmissionError> {
        let (state, _) = &*self.admission;
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed {
            return Err(ExecutionAdmissionError::Closed);
        }
        state.active += 1;
        drop(state);
        Ok(ExecutionWorkLease {
            admission: Arc::clone(&self.admission),
        })
    }

    pub(crate) fn drain(&self, control: &ExecutionDrainControl) -> ExecutionDrainOutcome {
        let (state, changed) = &*self.admission;
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
            let (next_state, wait_result) = changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|error| error.into_inner());
            state = next_state;
            if wait_result.timed_out() {
                return ExecutionDrainOutcome::TimedOut {
                    outstanding: ExecutionOutstandingWork {
                        active: state.active,
                    },
                };
            }
        }
    }

    pub(crate) fn cancel_and_drain(
        &self,
        control: &ExecutionDrainControl,
    ) -> ExecutionDrainOutcome {
        self.close_admission();
        self.drain(control)
    }
}

impl Drop for ExecutionWorkLease {
    fn drop(&mut self) {
        let (state, changed) = &*self.admission;
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        debug_assert!(state.active > 0);
        state.active = state.active.saturating_sub(1);
        drop(state);
        changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::identity::ExecutionSessionId;
    use std::time::Duration;

    fn state() -> ExecutionRuntimeState {
        ExecutionRuntimeState::new(
            ExecutionSessionId::new(uuid::Uuid::nil()),
            crate::execution::identity::RuntimeGeneration::INITIAL,
        )
    }

    #[test]
    fn closed_session_drains_an_active_lease_and_rejects_new_work() {
        let state = state();
        let lease = state.admit().expect("test admission must open");
        assert_eq!(
            state.cancel_and_drain(&ExecutionDrainControl::new(Instant::now())),
            ExecutionDrainOutcome::TimedOut {
                outstanding: ExecutionOutstandingWork { active: 1 },
            }
        );
        assert!(matches!(
            state.admit(),
            Err(ExecutionAdmissionError::Closed)
        ));

        drop(lease);
        assert_eq!(
            state.drain(&ExecutionDrainControl::new(
                Instant::now() + Duration::from_secs(1),
            )),
            ExecutionDrainOutcome::Drained {
                outstanding: ExecutionOutstandingWork { active: 0 },
            }
        );
    }
}
