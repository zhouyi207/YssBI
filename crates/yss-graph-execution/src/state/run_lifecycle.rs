//! Discharge the run transition obligation or fail the run when execution unwinds.
use crate::error::ExecutePreparedError;
use crate::run_registry::{RunId, RunRegistry, RunRegistryError, RunState};

pub(super) struct RunLifecycleGuard<'a> {
    registry: &'a RunRegistry,
    run_id: RunId,
    transition_pending: bool,
}

impl<'a> RunLifecycleGuard<'a> {
    pub(super) fn start(
        registry: &'a RunRegistry,
        run_id: RunId,
    ) -> Result<Self, RunRegistryError> {
        let guard = Self {
            registry,
            run_id,
            transition_pending: true,
        };
        registry.transition(run_id, RunState::Running)?;
        Ok(guard)
    }

    pub(super) fn begin_finalization(&mut self) -> Result<(), RunRegistryError> {
        self.registry
            .transition(self.run_id, RunState::Finalizing)?;
        self.transition_pending = false;
        Ok(())
    }

    pub(super) fn terminate(&mut self, error: ExecutePreparedError) -> ExecutePreparedError {
        let terminal = if matches!(&error, ExecutePreparedError::Cancelled { .. }) {
            RunState::Cancelled
        } else {
            RunState::Failed
        };
        match self.registry.transition(self.run_id, terminal) {
            Ok(()) => {
                self.transition_pending = false;
                error
            }
            Err(error) => ExecutePreparedError::RunRegistry(error),
        }
    }
}

impl Drop for RunLifecycleGuard<'_> {
    fn drop(&mut self) {
        if self.transition_pending {
            let _ = self.registry.transition(self.run_id, RunState::Failed);
        }
    }
}
