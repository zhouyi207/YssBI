use std::sync::Arc;
use std::time::Instant;

use thiserror::Error;
use yss_database_runtime::error::DatabaseError;
use yss_project::ProjectOperationError;

use super::replacement::ReplacementWorker;
use super::{ApplicationSessionRefreshError, ApplicationSessionSlot, SessionSlotState};

/// Work a failed replacement left on the old session, in the order recovery finishes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRecoveryPhase {
    DrainOldExecution,
    DrainOldDatabase,
    ClearOldProject,
}

#[derive(Debug, Error)]
pub enum SessionRecoveryError {
    #[error("session recovery deadline elapsed during {phase:?}")]
    DeadlineElapsed { phase: SessionRecoveryPhase },
    #[error("abandoned database storage handoffs could not be resolved")]
    DatabaseRecovery(#[source] DatabaseError),
    #[error("the retained Project state could not be cleared")]
    ProjectClear(#[source] ProjectOperationError),
    #[error("application session epoch is exhausted")]
    EpochExhausted,
    #[error("the recovered application session could not be published")]
    Publish(#[source] ApplicationSessionRefreshError),
}

impl ApplicationSessionSlot {
    pub(super) fn recover(&self, deadline: Instant) -> Result<bool, SessionRecoveryError> {
        let Some(mut worker) = self.resume_recovery()? else {
            return Ok(false);
        };
        worker.drain_old_session(deadline)?;
        worker
            .old
            .project()
            .clear_project()
            .map_err(SessionRecoveryError::ProjectClear)?;
        let project = Arc::new(worker.old.project().clone());
        self.publish_successor(worker, project)
            .map_err(SessionRecoveryError::Publish)?;
        Ok(true)
    }

    /// Hands the retained session to one worker, so a concurrent caller observes
    /// `Replacing` instead of running the same recovery twice.
    fn resume_recovery(&self) -> Result<Option<ReplacementWorker>, SessionRecoveryError> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let (epoch, old, pending) = match &*state {
            // A dropped candidate may already carry the failed epoch.
            SessionSlotState::Recovering {
                epoch,
                old,
                pending,
            } => (
                epoch.next().ok_or(SessionRecoveryError::EpochExhausted)?,
                Arc::clone(old),
                *pending,
            ),
            SessionSlotState::Inactive { .. }
            | SessionSlotState::Replacing { .. }
            | SessionSlotState::Active(_) => return Ok(None),
        };
        *state = SessionSlotState::Replacing { epoch };
        drop(state);
        Ok(Some(ReplacementWorker::new(
            Arc::clone(&self.state),
            epoch,
            old,
            pending,
        )))
    }
}
