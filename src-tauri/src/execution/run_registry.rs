use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

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
    states: Mutex<BTreeMap<RunId, RunState>>,
    next_id: AtomicU64,
}

impl RunRegistry {
    pub fn new() -> Self {
        Self {
            states: Mutex::new(BTreeMap::new()),
            next_id: AtomicU64::new(0),
        }
    }

    pub(crate) fn admit_next(&self) -> Result<RunId, RunRegistryError> {
        let value = self
            .next_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| RunRegistryError::Exhausted)?;
        let run = RunId::from_existing(value);
        self.admit(run)?;
        Ok(run)
    }

    pub fn admit(&self, run: RunId) -> Result<(), RunRegistryError> {
        let mut states = self
            .states
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if states.insert(run, RunState::Admitted).is_some() {
            return Err(RunRegistryError::Duplicate);
        }
        Ok(())
    }

    pub fn state(&self, run: RunId) -> Option<RunState> {
        self.states
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&run)
            .copied()
    }

    pub fn transition(&self, run: RunId, next: RunState) -> Result<(), RunRegistryError> {
        let mut states = self
            .states
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let current = states.get_mut(&run).ok_or(RunRegistryError::Missing)?;
        let valid = matches!(
            (*current, next),
            (RunState::Admitted, RunState::Running)
                | (RunState::Admitted, RunState::Cancelled)
                | (RunState::Admitted, RunState::Failed)
                | (RunState::Running, RunState::Finalizing)
                | (RunState::Running, RunState::Cancelled)
                | (RunState::Running, RunState::Failed)
                | (RunState::Finalizing, RunState::Succeeded)
                | (RunState::Finalizing, RunState::Failed)
        );
        if !valid {
            return Err(RunRegistryError::InvalidTransition);
        }
        *current = next;
        Ok(())
    }
}

impl Default for RunRegistry {
    fn default() -> Self {
        Self::new()
    }
}
