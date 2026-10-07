use std::sync::Arc;
use std::time::Instant;

use thiserror::Error;
use yss_database_runtime::runtime::{
    DatabaseDrainDeadline, DatabaseDrainOutcome, DatabaseSessionDrainControl,
};
use yss_graph_execution::state::{ExecutionDrainControl, ExecutionDrainOutcome};
use yss_project::ProjectState;

use super::recovery::{
    RecoveryRequired, RecoveryWorkState, RetainedSessionRecovery, SessionRecoveryError,
    SessionRecoveryId, SessionRecoveryPhase, recovery_required,
};
use super::{
    ApplicationSession, ApplicationSessionEpoch, ApplicationSessionRefreshError,
    ApplicationSessionSlot, SessionSlotInner, SessionSlotState,
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
    #[error("application session replacement worker is stale")]
    StaleWorker,
    #[error("application session replacement phase is not current")]
    WrongPhase,
    #[error("candidate construction is not installed in the staged slot")]
    CandidateConstructionUnavailable,
    #[error("application session recovery is required")]
    RecoveryRequired(RecoveryRequired),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReplacementPhase {
    CloseAdmissions,
    DrainExecution,
    DrainDatabase,
    ClearProject,
    HydrateProject,
    BuildCandidate,
    PublishCandidate,
}

impl ReplacementPhase {
    fn next(self) -> Option<Self> {
        match self {
            Self::CloseAdmissions => Some(Self::DrainExecution),
            Self::DrainExecution => Some(Self::DrainDatabase),
            Self::DrainDatabase => Some(Self::ClearProject),
            Self::ClearProject => Some(Self::HydrateProject),
            Self::HydrateProject => Some(Self::BuildCandidate),
            Self::BuildCandidate => Some(Self::PublishCandidate),
            Self::PublishCandidate => None,
        }
    }

    fn recovery_phase(self) -> SessionRecoveryPhase {
        match self {
            Self::CloseAdmissions | Self::DrainExecution => SessionRecoveryPhase::DrainOldExecution,
            Self::DrainDatabase => SessionRecoveryPhase::DrainOldDatabase,
            Self::ClearProject
            | Self::HydrateProject
            | Self::BuildCandidate
            | Self::PublishCandidate => SessionRecoveryPhase::ClearOldProject,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReplacementAdvanceOutcome {
    Advanced(ReplacementPhase),
    Superseded(RecoveryRequired),
}

pub(super) struct ReplacementWorker {
    slot: Arc<SessionSlotInner>,
    pub(super) epoch: ApplicationSessionEpoch,
    phase: ReplacementPhase,
    old: Option<Arc<ApplicationSession>>,
    completed: bool,
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

impl ReplacementWorker {
    #[cfg(test)]
    pub(super) fn complete_phase_for_test(
        &mut self,
        completed: ReplacementPhase,
    ) -> Result<ReplacementAdvanceOutcome, SessionReplacementError> {
        self.complete_phase(completed)
    }

    fn complete_phase(
        &mut self,
        completed: ReplacementPhase,
    ) -> Result<ReplacementAdvanceOutcome, SessionReplacementError> {
        if self.phase != completed {
            return Err(SessionReplacementError::WrongPhase);
        }

        let mut state = self
            .slot
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        match &mut *state {
            SessionSlotState::Replacing { epoch, phase }
                if *epoch == self.epoch && *phase == completed =>
            {
                let Some(next) = completed.next() else {
                    return Err(SessionReplacementError::CandidateConstructionUnavailable);
                };
                *phase = next;
                self.phase = next;
                Ok(ReplacementAdvanceOutcome::Advanced(next))
            }
            SessionSlotState::Recovering {
                epoch,
                recovery,
                retained,
            } if *epoch == self.epoch => {
                let required = recovery_required(*epoch, *recovery, retained);
                self.completed = true;
                self.old.take();
                Ok(ReplacementAdvanceOutcome::Superseded(required))
            }
            SessionSlotState::Replacing { epoch, .. } if *epoch != self.epoch => {
                Err(SessionReplacementError::StaleWorker)
            }
            SessionSlotState::Recovering { .. }
            | SessionSlotState::Inactive { .. }
            | SessionSlotState::Active(_) => Err(SessionReplacementError::StaleWorker),
            SessionSlotState::Replacing { .. } => Err(SessionReplacementError::WrongPhase),
        }
    }

    fn retain_recovery(
        &mut self,
        phase: SessionRecoveryPhase,
    ) -> Result<RecoveryRequired, SessionRecoveryError> {
        let mut state = self
            .slot
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        match &*state {
            SessionSlotState::Replacing {
                epoch,
                phase: current_phase,
            } if *epoch == self.epoch && *current_phase == self.phase => {}
            SessionSlotState::Replacing { epoch, .. } if *epoch != self.epoch => {
                return Err(SessionRecoveryError::StaleEpoch);
            }
            SessionSlotState::Replacing { .. } => return Err(SessionRecoveryError::WrongPhase),
            SessionSlotState::Recovering { .. } => return Err(SessionRecoveryError::StaleEpoch),
            SessionSlotState::Inactive { .. } | SessionSlotState::Active(_) => {
                return Err(SessionRecoveryError::StaleEpoch);
            }
        }

        let Some(old) = self.old.take() else {
            return Err(SessionRecoveryError::StaleEpoch);
        };
        let recovery = SessionRecoveryId::from_existing(self.epoch.get());
        *state = SessionSlotState::Recovering {
            epoch: self.epoch,
            recovery,
            retained: RetainedSessionRecovery {
                old,
                work: RecoveryWorkState::Available(phase),
            },
        };
        self.completed = true;
        Ok(RecoveryRequired {
            recovery,
            failed_epoch: self.epoch,
            phase,
        })
    }
}

impl Drop for ReplacementWorker {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let Some(old) = self.old.take() else {
            return;
        };
        let mut state = self
            .slot
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if matches!(
            &*state,
            SessionSlotState::Replacing { epoch, phase }
                if *epoch == self.epoch && *phase == self.phase
        ) {
            let recovery = SessionRecoveryId::from_existing(self.epoch.get());
            *state = SessionSlotState::Recovering {
                epoch: self.epoch,
                recovery,
                retained: RetainedSessionRecovery {
                    old,
                    work: RecoveryWorkState::Available(self.phase.recovery_phase()),
                },
            };
        }
    }
}

impl ApplicationSessionSlot {
    pub(super) fn begin_project_replacement(
        &self,
        captured: &Arc<ApplicationSession>,
    ) -> Result<ProjectReplacement, SessionReplacementError> {
        let mut worker = self.begin_replacement(captured)?;
        let old = worker
            .old
            .as_ref()
            .cloned()
            .ok_or(SessionReplacementError::StaleWorker)?;
        let deadline = Instant::now() + std::time::Duration::from_secs(30);

        if !matches!(
            old.execution()
                .cancel_and_drain(&ExecutionDrainControl::new(deadline)),
            ExecutionDrainOutcome::Drained { .. }
        ) {
            let required = worker
                .retain_recovery(SessionRecoveryPhase::DrainOldExecution)
                .map_err(|_| SessionReplacementError::StaleWorker)?;
            return Err(SessionReplacementError::RecoveryRequired(required));
        }
        worker.complete_phase(ReplacementPhase::CloseAdmissions)?;
        worker.complete_phase(ReplacementPhase::DrainExecution)?;

        old.database().close_admission();
        if !matches!(
            old.database().drain(&DatabaseSessionDrainControl::new(
                DatabaseDrainDeadline::at(deadline)
            )),
            DatabaseDrainOutcome::Drained { .. }
        ) {
            let required = worker
                .retain_recovery(SessionRecoveryPhase::DrainOldDatabase)
                .map_err(|_| SessionReplacementError::StaleWorker)?;
            return Err(SessionReplacementError::RecoveryRequired(required));
        }
        worker.complete_phase(ReplacementPhase::DrainDatabase)?;

        Ok(ProjectReplacement {
            project: Arc::new(old.project().clone()),
            worker,
        })
    }

    pub(super) fn finish_project_replacement(
        &self,
        mut replacement: ProjectReplacement,
    ) -> Result<(), ApplicationSessionRefreshError> {
        replacement
            .worker
            .complete_phase(ReplacementPhase::ClearProject)
            .map_err(|_| ApplicationSessionRefreshError::Replacement)?;
        replacement
            .worker
            .complete_phase(ReplacementPhase::HydrateProject)
            .map_err(|_| ApplicationSessionRefreshError::Replacement)?;

        let reusable_instances = replacement
            .worker
            .old
            .as_ref()
            .map(|old| old.database().instances_for_replacement())
            .unwrap_or_default();
        let candidate = build_current_project_candidate(
            replacement.worker.epoch,
            replacement.project,
            reusable_instances,
            &self.nodes,
        )
        .map_err(|_| ApplicationSessionRefreshError::Candidate)?;

        replacement
            .worker
            .complete_phase(ReplacementPhase::BuildCandidate)
            .map_err(|_| ApplicationSessionRefreshError::Replacement)?;

        let session = candidate.into_session();
        {
            let mut state = self
                .inner
                .state
                .write()
                .unwrap_or_else(|error| error.into_inner());
            let SessionSlotState::Replacing { epoch, phase } = &*state else {
                return Err(ApplicationSessionRefreshError::Replacement);
            };
            if *epoch != replacement.worker.epoch || *phase != ReplacementPhase::PublishCandidate {
                return Err(ApplicationSessionRefreshError::Replacement);
            }
            if session.epoch() != *epoch {
                return Err(ApplicationSessionRefreshError::Replacement);
            }
            *state = SessionSlotState::Active(Arc::new(session));
            replacement.worker.completed = true;
        }
        if let Some(old) = replacement.worker.old.take() {
            old.presentation.session_changed();
        }
        Ok(())
    }

    fn begin_replacement(
        &self,
        captured: &Arc<ApplicationSession>,
    ) -> Result<ReplacementWorker, SessionReplacementError> {
        let mut state = self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let (old, epoch) = match &*state {
            SessionSlotState::Active(session) => {
                if !Arc::ptr_eq(session, captured) {
                    return Err(SessionReplacementError::StaleSession);
                }
                let Some(epoch) = session.epoch().next() else {
                    return Err(SessionReplacementError::EpochExhausted);
                };
                (Arc::clone(session), epoch)
            }
            SessionSlotState::Inactive { .. } => return Err(SessionReplacementError::Inactive),
            SessionSlotState::Replacing { .. } => {
                return Err(SessionReplacementError::Replacing);
            }
            SessionSlotState::Recovering { .. } => {
                return Err(SessionReplacementError::Recovering);
            }
        };
        let phase = ReplacementPhase::CloseAdmissions;
        *state = SessionSlotState::Replacing { epoch, phase };
        Ok(ReplacementWorker {
            slot: Arc::clone(&self.inner),
            epoch,
            phase,
            old: Some(old),
            completed: false,
        })
    }

    #[cfg(test)]
    pub(super) fn begin_replacement_for_test(
        &self,
        captured: &Arc<ApplicationSession>,
    ) -> Result<ReplacementWorker, SessionReplacementError> {
        self.begin_replacement(captured)
    }

    #[cfg(test)]
    pub(super) fn set_replacing_for_test(
        &self,
        epoch: ApplicationSessionEpoch,
        phase: ReplacementPhase,
    ) {
        *self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner()) =
            SessionSlotState::Replacing { epoch, phase };
    }
}
