use std::sync::{Arc, Condvar, Mutex, PoisonError};

use crate::execution::identity::{ExecutionSessionId, RuntimeGeneration};
use crate::execution::result_store::ResultStore;
use crate::execution::run_registry::RunRegistry;

#[derive(Default)]
struct RuntimeAdmission {
    closed: bool,
    active: usize,
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

    pub fn close_admission(&self) {
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

    #[cfg(test)]
    pub(crate) fn admit_for_test(&self) -> bool {
        let (state, _) = &*self.admission;
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed {
            return false;
        }
        state.active += 1;
        true
    }

    #[cfg(test)]
    pub(crate) fn release_for_test(&self) {
        let (state, changed) = &*self.admission;
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        state.active = state.active.saturating_sub(1);
        drop(state);
        changed.notify_all();
    }
}
