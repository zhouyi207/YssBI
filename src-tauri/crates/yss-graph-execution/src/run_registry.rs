use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RunId(u64);

impl RunId {
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunState {
    Admitted,
    Running,
    Finalizing,
    Succeeded,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionCancelOutcome {
    NotFound,
    AlreadyCancelled,
    AlreadyTerminal,
    Requested,
}

#[derive(Debug, Error)]
pub enum RunRegistryError {
    #[error("run id is already registered")]
    Duplicate,
    #[error("run id is not registered")]
    Missing,
    #[error("run state transition is invalid")]
    InvalidTransition,
    #[error("run id space is exhausted")]
    Exhausted,
}

pub struct RunRegistry {
    records: Mutex<RunRecords>,
    next_id: AtomicU64,
}

const TERMINAL_RETENTION: usize = 1024;

#[derive(Default)]
struct RunRecords {
    runs: BTreeMap<RunId, RunRecord>,
    /// Completion order, independent of admission/RunId order.
    terminal: VecDeque<RunId>,
}

struct RunRecord {
    state: RunState,
    cancellation: Option<Arc<AtomicBool>>,
}

impl RunRegistry {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(RunRecords::default()),
            next_id: AtomicU64::new(1),
        }
    }

    pub(crate) fn admit_next(
        &self,
        cancellation: Option<Arc<AtomicBool>>,
    ) -> Result<RunId, RunRegistryError> {
        let value = self
            .next_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| RunRegistryError::Exhausted)?;
        let run = RunId::from_existing(value);
        self.admit_record(run, cancellation)?;
        Ok(run)
    }

    pub fn admit(&self, run: RunId) -> Result<(), RunRegistryError> {
        self.admit_record(run, None)
    }

    fn admit_record(
        &self,
        run: RunId,
        cancellation: Option<Arc<AtomicBool>>,
    ) -> Result<(), RunRegistryError> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match records.runs.entry(run) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(RunRecord {
                    state: RunState::Admitted,
                    cancellation,
                });
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => Err(RunRegistryError::Duplicate),
        }
    }

    pub fn state(&self, run: RunId) -> Option<RunState> {
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .runs
            .get(&run)
            .map(|record| record.state)
    }

    pub(crate) fn cancel(&self, run: RunId) -> ExecutionCancelOutcome {
        let records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(record) = records.runs.get(&run) else {
            return ExecutionCancelOutcome::NotFound;
        };
        match record.state {
            RunState::Cancelled => ExecutionCancelOutcome::AlreadyCancelled,
            RunState::Succeeded | RunState::Failed => ExecutionCancelOutcome::AlreadyTerminal,
            RunState::Admitted | RunState::Running | RunState::Finalizing => {
                if let Some(control) = &record.cancellation {
                    control.store(true, Ordering::Release);
                }
                ExecutionCancelOutcome::Requested
            }
        }
    }

    pub(crate) fn cancel_all(&self) {
        let records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for record in records.runs.values() {
            if let Some(control) = &record.cancellation {
                control.store(true, Ordering::Release);
            }
        }
    }

    pub fn transition(&self, run: RunId, next: RunState) -> Result<(), RunRegistryError> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let current = records
            .runs
            .get_mut(&run)
            .ok_or(RunRegistryError::Missing)?;
        let valid = matches!(
            (current.state, next),
            (RunState::Admitted, RunState::Running)
                | (RunState::Admitted, RunState::Cancelled)
                | (RunState::Admitted, RunState::Failed)
                | (RunState::Running, RunState::Finalizing)
                | (RunState::Running, RunState::Cancelled)
                | (RunState::Running, RunState::Failed)
                | (RunState::Finalizing, RunState::Succeeded)
                | (RunState::Finalizing, RunState::Cancelled)
                | (RunState::Finalizing, RunState::Failed)
        );
        if !valid {
            return Err(RunRegistryError::InvalidTransition);
        }
        current.state = next;
        if matches!(
            next,
            RunState::Succeeded | RunState::Cancelled | RunState::Failed
        ) {
            current.cancellation = None;
            records.terminal.push_back(run);
            while records.terminal.len() > TERMINAL_RETENTION {
                if let Some(expired) = records.terminal.pop_front() {
                    records.runs.remove(&expired);
                }
            }
        }
        Ok(())
    }
}

impl Default for RunRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_uses_completion_order_and_preserves_active_and_finalizing_runs() {
        let registry = RunRegistry::new();
        let admitted = registry.admit_next(None).unwrap();
        let running = registry.admit_next(None).unwrap();
        registry.transition(running, RunState::Running).unwrap();
        let finalizing = registry.admit_next(None).unwrap();
        registry.transition(finalizing, RunState::Running).unwrap();
        registry
            .transition(finalizing, RunState::Finalizing)
            .unwrap();
        let older_id = registry.admit_next(None).unwrap();
        let first_completed = registry.admit_next(None).unwrap();
        registry
            .transition(first_completed, RunState::Cancelled)
            .unwrap();
        registry.transition(older_id, RunState::Failed).unwrap();
        for _ in 0..TERMINAL_RETENTION - 1 {
            let run = registry.admit_next(None).unwrap();
            registry.transition(run, RunState::Running).unwrap();
            registry.transition(run, RunState::Finalizing).unwrap();
            registry.transition(run, RunState::Succeeded).unwrap();
        }
        assert_eq!(registry.state(first_completed), None);
        assert_eq!(registry.state(older_id), Some(RunState::Failed));
        assert_eq!(registry.state(admitted), Some(RunState::Admitted));
        assert_eq!(registry.state(running), Some(RunState::Running));
        assert_eq!(registry.state(finalizing), Some(RunState::Finalizing));
        assert_eq!(
            registry.records.lock().unwrap().runs.len(),
            TERMINAL_RETENTION + 3
        );
    }

    #[test]
    fn duplicate_admission_preserves_the_active_run_state() {
        let registry = RunRegistry::new();

        let run_id = registry
            .admit_next(None)
            .expect("the first run is admitted");

        assert!(run_id.get() > 0);
        assert_eq!(registry.state(run_id), Some(RunState::Admitted));
        registry.transition(run_id, RunState::Running).unwrap();
        assert!(matches!(
            registry.admit(run_id),
            Err(RunRegistryError::Duplicate)
        ));
        assert_eq!(registry.state(run_id), Some(RunState::Running));
    }
}
