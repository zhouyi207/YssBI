use std::sync::{Arc, RwLock};
use std::time::Instant;

use thiserror::Error;
use yss_database_runtime::runtime::{
    DatabaseDrainDeadline, DatabaseDrainOutcome, DatabaseSessionDrainControl,
};
use yss_graph_execution::state::{ExecutionDrainControl, ExecutionDrainOutcome};
use yss_project::ProjectState;

use super::recovery::{SessionRecoveryError, SessionRecoveryPhase};
use super::{
    ApplicationSession, ApplicationSessionEpoch, ApplicationSessionRefreshError,
    ApplicationSessionSlot, SessionSlotState,
};
use crate::session::factory::build_current_project_candidate;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub(crate) enum SessionReplacementError {
    #[error("application session is inactive")]
    Inactive,
    #[error("application session replacement is already in progress")]
    Replacing,
    #[error("application session recovery is required")]
    Recovering,
    #[error("application session epoch is exhausted")]
    EpochExhausted,
    #[error("captured application session is no longer current")]
    StaleSession,
    #[error("the old application session did not drain and is retained for recovery")]
    RecoveryRequired,
}

/// Owns the old session while the slot is `Replacing`. Dropping it before publication
/// retains the old session in `Recovering`, resuming at the work still `pending`.
pub(super) struct ReplacementWorker {
    state: Arc<RwLock<SessionSlotState>>,
    epoch: ApplicationSessionEpoch,
    pub(super) old: Arc<ApplicationSession>,
    pending: SessionRecoveryPhase,
    published: bool,
}

impl ReplacementWorker {
    pub(super) fn new(
        state: Arc<RwLock<SessionSlotState>>,
        epoch: ApplicationSessionEpoch,
        old: Arc<ApplicationSession>,
        pending: SessionRecoveryPhase,
    ) -> Self {
        Self {
            state,
            epoch,
            old,
            pending,
            published: false,
        }
    }

    /// Records each drained stage, so an abandoned worker leaves recovery only the rest.
    pub(super) fn drain_old_session(
        &mut self,
        deadline: Instant,
    ) -> Result<(), SessionRecoveryError> {
        if self.pending == SessionRecoveryPhase::DrainOldExecution {
            if let ExecutionDrainOutcome::TimedOut { .. } = self
                .old
                .execution()
                .cancel_and_drain(&ExecutionDrainControl::new(deadline))
            {
                return Err(SessionRecoveryError::DeadlineElapsed {
                    phase: self.pending,
                });
            }
            self.pending = SessionRecoveryPhase::DrainOldDatabase;
        }
        if self.pending == SessionRecoveryPhase::DrainOldDatabase {
            let database = self.old.database();
            database.close_admission();
            // Abandoned storage handoffs count as outstanding work, and they may only be
            // resolved once admission is closed; the drain cannot finish before that.
            database
                .resolve_storage_recoveries()
                .map_err(SessionRecoveryError::DatabaseRecovery)?;
            if let DatabaseDrainOutcome::TimedOut { .. } = database.drain(
                &DatabaseSessionDrainControl::new(DatabaseDrainDeadline::at(deadline)),
            ) {
                return Err(SessionRecoveryError::DeadlineElapsed {
                    phase: self.pending,
                });
            }
            self.pending = SessionRecoveryPhase::ClearOldProject;
        }
        Ok(())
    }
}

impl Drop for ReplacementWorker {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if matches!(&*state, SessionSlotState::Replacing { epoch } if *epoch == self.epoch) {
            *state = SessionSlotState::Recovering {
                epoch: self.epoch,
                old: Arc::clone(&self.old),
                pending: self.pending,
            };
        }
    }
}

/// A replacement that has closed the old session's admission and drained its
/// Execution/Database work. The Project authority may perform its lifecycle
/// I/O while this value is held; dropping it retains the old session in
/// `Recovering` instead of publishing a partial candidate.
pub(crate) struct ProjectReplacement {
    worker: ReplacementWorker,
    project: Arc<ProjectState>,
}

impl ProjectReplacement {
    pub(crate) fn project(&self) -> &ProjectState {
        &self.project
    }
}

impl ApplicationSessionSlot {
    pub(super) fn begin_project_replacement(
        &self,
        captured: &Arc<ApplicationSession>,
        deadline: Instant,
    ) -> Result<ProjectReplacement, SessionReplacementError> {
        let mut worker = self.begin_replacement(captured)?;
        worker
            .drain_old_session(deadline)
            .map_err(|_| SessionReplacementError::RecoveryRequired)?;
        Ok(ProjectReplacement {
            project: Arc::new(worker.old.project().clone()),
            worker,
        })
    }

    pub(super) fn finish_project_replacement(
        &self,
        replacement: ProjectReplacement,
    ) -> Result<(), ApplicationSessionRefreshError> {
        self.publish_successor(replacement.worker, replacement.project)
    }

    /// Builds the session for `project` and publishes it in place of the worker's old session.
    pub(super) fn publish_successor(
        &self,
        mut worker: ReplacementWorker,
        project: Arc<ProjectState>,
    ) -> Result<(), ApplicationSessionRefreshError> {
        let candidate = build_current_project_candidate(
            worker.epoch,
            project,
            worker.old.database().instances_for_replacement(),
            &self.nodes,
        )
        .map_err(|_| ApplicationSessionRefreshError::Candidate)?;
        let session = Arc::new(candidate.into_session());
        {
            let mut state = self
                .state
                .write()
                .unwrap_or_else(|error| error.into_inner());
            if !matches!(&*state, SessionSlotState::Replacing { epoch } if *epoch == worker.epoch) {
                return Err(ApplicationSessionRefreshError::Replacement);
            }
            *state = SessionSlotState::Active(session);
            worker.published = true;
        }
        // Observers may capture the published session as soon as they are told.
        worker.old.presentation.session_changed();
        Ok(())
    }

    fn begin_replacement(
        &self,
        captured: &Arc<ApplicationSession>,
    ) -> Result<ReplacementWorker, SessionReplacementError> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let epoch = match &*state {
            SessionSlotState::Active(session) if Arc::ptr_eq(session, captured) => session
                .epoch()
                .next()
                .ok_or(SessionReplacementError::EpochExhausted)?,
            SessionSlotState::Active(_) => return Err(SessionReplacementError::StaleSession),
            SessionSlotState::Inactive { .. } => return Err(SessionReplacementError::Inactive),
            SessionSlotState::Replacing { .. } => return Err(SessionReplacementError::Replacing),
            SessionSlotState::Recovering { .. } => {
                return Err(SessionReplacementError::Recovering);
            }
        };
        *state = SessionSlotState::Replacing { epoch };
        Ok(ReplacementWorker::new(
            Arc::clone(&self.state),
            epoch,
            Arc::clone(captured),
            SessionRecoveryPhase::DrainOldExecution,
        ))
    }
}
