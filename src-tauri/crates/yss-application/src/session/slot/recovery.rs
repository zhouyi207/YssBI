use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use thiserror::Error;
use yss_database_runtime::runtime::{
    DatabaseDrainDeadline, DatabaseDrainOutcome, DatabaseSessionDrainControl,
};
use yss_graph_execution::state::{ExecutionDrainControl, ExecutionDrainOutcome};

use super::{
    ApplicationSession, ApplicationSessionEpoch, ApplicationSessionSlot, SessionSlotInner,
    SessionSlotState,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SessionRecoveryId(u64);

impl SessionRecoveryId {
    pub(super) const fn from_existing(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SessionRecoveryPhase {
    DrainOldExecution,
    RetryDatabaseCompensation,
    ResolveOldDatabase,
    DrainOldDatabase,
    ClearOldProject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionRecoveryDeadline(Instant);

impl SessionRecoveryDeadline {
    pub const fn at(instant: Instant) -> Self {
        Self(instant)
    }

    pub fn is_expired(self) -> bool {
        Instant::now() >= self.0
    }
}

pub struct SessionRecoveryControl {
    deadline: SessionRecoveryDeadline,
}

impl SessionRecoveryControl {
    pub const fn new(deadline: SessionRecoveryDeadline) -> Self {
        Self { deadline }
    }

    pub const fn deadline(&self) -> SessionRecoveryDeadline {
        self.deadline
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryRequired {
    pub recovery: SessionRecoveryId,
    pub failed_epoch: ApplicationSessionEpoch,
    pub phase: SessionRecoveryPhase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRecoveryOutcome {
    ReplacementMayRestart { next_epoch: ApplicationSessionEpoch },
    RetryRequired(RecoveryRequired),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum SessionRecoveryError {
    #[error("session recovery was not found")]
    NotFound,
    #[error("session recovery belongs to another epoch")]
    StaleEpoch,
    #[error("session recovery is already in progress")]
    AlreadyInProgress,
    #[error("session recovery is in the wrong phase")]
    WrongPhase,
    #[error("session recovery authority is ambiguous")]
    AuthorityAmbiguous,
    #[error("database recovery claim failed")]
    DatabaseClaim,
    #[error("database compensation failed")]
    DatabaseCompensation,
    #[error("session recovery attempt identifiers are exhausted")]
    AttemptIdExhausted,
    #[error("session recovery deadline elapsed during {phase:?}")]
    DeadlineElapsed { phase: SessionRecoveryPhase },
    #[error("session recovery claim is no longer current")]
    StaleClaim,
    #[error("session recovery claim has already been consumed")]
    ClaimConsumed,
    #[error("the retained Project state could not be cleared during recovery")]
    ProjectClearFailed,
}

pub(super) struct RetainedSessionRecovery {
    pub(super) old: Arc<ApplicationSession>,
    pub(super) work: RecoveryWorkState,
}

pub(super) enum RecoveryWorkState {
    Available(SessionRecoveryPhase),
    InProgress {
        attempt: SessionRecoveryAttemptId,
        phase: SessionRecoveryPhase,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SessionRecoveryAttemptId(u64);

pub(super) struct SessionRecoveryClaimGuard {
    slot: Arc<SessionSlotInner>,
    epoch: ApplicationSessionEpoch,
    recovery: SessionRecoveryId,
    attempt: SessionRecoveryAttemptId,
    pub(super) old: Arc<ApplicationSession>,
    phase: SessionRecoveryPhase,
    work: Option<SessionRecoveryPhase>,
}

impl SessionSlotInner {
    fn next_attempt(&self) -> Result<SessionRecoveryAttemptId, SessionRecoveryError> {
        self.next_attempt
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .map(SessionRecoveryAttemptId)
            .map_err(|_| SessionRecoveryError::AttemptIdExhausted)
    }
}

impl SessionRecoveryClaimGuard {
    pub(super) fn phase(&self) -> SessionRecoveryPhase {
        self.phase
    }

    fn finish(
        mut self,
        next: Option<SessionRecoveryPhase>,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        let Some(phase) = self.work.take() else {
            return Err(SessionRecoveryError::ClaimConsumed);
        };

        let mut state = self
            .slot
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let matches_claim = matches!(
            &*state,
            SessionSlotState::Recovering {
                epoch,
                recovery,
                retained: RetainedSessionRecovery {
                    work: RecoveryWorkState::InProgress {
                        attempt,
                        phase: current_phase,
                    },
                    ..
                },
            } if *epoch == self.epoch
                && *recovery == self.recovery
                && *attempt == self.attempt
                && *current_phase == phase
        );
        if !matches_claim {
            self.work = Some(phase);
            return Err(SessionRecoveryError::StaleClaim);
        }

        if next.is_none() && self.epoch.next().is_none() {
            self.work = Some(phase);
            return Err(SessionRecoveryError::StaleEpoch);
        }

        let outcome = match next {
            Some(next_phase) => {
                let SessionSlotState::Recovering { retained, .. } = &mut *state else {
                    self.work = Some(phase);
                    return Err(SessionRecoveryError::StaleClaim);
                };
                retained.work = RecoveryWorkState::Available(next_phase);
                SessionRecoveryOutcome::RetryRequired(RecoveryRequired {
                    recovery: self.recovery,
                    failed_epoch: self.epoch,
                    phase: next_phase,
                })
            }
            None => {
                let Some(next_epoch) = self.epoch.next() else {
                    self.work = Some(phase);
                    return Err(SessionRecoveryError::StaleEpoch);
                };
                *state = SessionSlotState::Inactive { next_epoch };
                SessionRecoveryOutcome::ReplacementMayRestart { next_epoch }
            }
        };
        Ok(outcome)
    }

    #[cfg(test)]
    fn finish_for_test(
        self,
        next: Option<SessionRecoveryPhase>,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        self.finish(next)
    }
}

impl Drop for SessionRecoveryClaimGuard {
    fn drop(&mut self) {
        let Some(phase) = self.work.take() else {
            return;
        };
        let mut state = self
            .slot
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let SessionSlotState::Recovering {
            epoch,
            recovery,
            retained,
        } = &mut *state
        {
            let exact_claim = matches!(
                &retained.work,
                RecoveryWorkState::InProgress {
                    attempt,
                    phase: current_phase,
                } if *attempt == self.attempt
                    && *current_phase == phase
            );
            if *epoch == self.epoch && *recovery == self.recovery && exact_claim {
                retained.work = RecoveryWorkState::Available(phase);
            }
        }
    }
}

pub(super) fn recovery_required(
    epoch: ApplicationSessionEpoch,
    recovery: SessionRecoveryId,
    retained: &RetainedSessionRecovery,
) -> RecoveryRequired {
    let phase = match &retained.work {
        RecoveryWorkState::Available(phase) | RecoveryWorkState::InProgress { phase, .. } => *phase,
    };
    RecoveryRequired {
        recovery,
        failed_epoch: epoch,
        phase,
    }
}

impl ApplicationSessionSlot {
    pub(super) fn claim_recovery(
        &self,
        recovery: SessionRecoveryId,
    ) -> Result<SessionRecoveryClaimGuard, SessionRecoveryError> {
        let mut state = self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let SessionSlotState::Recovering {
            epoch,
            recovery: current_recovery,
            retained,
        } = &mut *state
        else {
            return Err(SessionRecoveryError::NotFound);
        };
        if *current_recovery != recovery {
            return Err(SessionRecoveryError::NotFound);
        }
        let phase = match &retained.work {
            RecoveryWorkState::Available(phase) => *phase,
            RecoveryWorkState::InProgress { .. } => {
                return Err(SessionRecoveryError::AlreadyInProgress);
            }
        };
        let attempt = self.inner.next_attempt()?;
        retained.work = RecoveryWorkState::InProgress { attempt, phase };
        Ok(SessionRecoveryClaimGuard {
            slot: Arc::clone(&self.inner),
            epoch: *epoch,
            recovery,
            attempt,
            old: Arc::clone(&retained.old),
            phase,
            work: Some(phase),
        })
    }

    pub(super) fn retry_session_recovery(
        &self,
        recovery: SessionRecoveryId,
        control: &SessionRecoveryControl,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        let guard = self.claim_recovery(recovery)?;
        let phase = guard.phase();
        if control.deadline().is_expired() {
            return Err(SessionRecoveryError::DeadlineElapsed { phase });
        }
        match phase {
            SessionRecoveryPhase::DrainOldExecution => {
                match guard
                    .old
                    .execution()
                    .cancel_and_drain(&ExecutionDrainControl::new(control.deadline().0))
                {
                    ExecutionDrainOutcome::Drained { .. } => {
                        guard.finish(Some(SessionRecoveryPhase::DrainOldDatabase))
                    }
                    ExecutionDrainOutcome::TimedOut { .. } => {
                        Err(SessionRecoveryError::DeadlineElapsed { phase })
                    }
                }
            }
            SessionRecoveryPhase::DrainOldDatabase => {
                guard.old.database().close_admission();
                match guard
                    .old
                    .database()
                    .drain(&DatabaseSessionDrainControl::new(
                        DatabaseDrainDeadline::at(control.deadline().0),
                    )) {
                    DatabaseDrainOutcome::Drained { .. } => {
                        guard.finish(Some(SessionRecoveryPhase::ClearOldProject))
                    }
                    DatabaseDrainOutcome::TimedOut { .. } => {
                        Err(SessionRecoveryError::DeadlineElapsed { phase })
                    }
                }
            }
            SessionRecoveryPhase::ClearOldProject => {
                guard
                    .old
                    .project()
                    .clear_project()
                    .map_err(|_| SessionRecoveryError::ProjectClearFailed)?;
                guard.finish(None)
            }
            SessionRecoveryPhase::RetryDatabaseCompensation
            | SessionRecoveryPhase::ResolveOldDatabase => Err(SessionRecoveryError::WrongPhase),
        }
    }

    pub(super) fn resolve_session_database_recovery(
        &self,
        recovery: SessionRecoveryId,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        let guard = self.claim_recovery(recovery)?;
        let phase = guard.phase();
        if !matches!(
            phase,
            SessionRecoveryPhase::RetryDatabaseCompensation
                | SessionRecoveryPhase::ResolveOldDatabase
        ) {
            return Err(SessionRecoveryError::WrongPhase);
        }
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        guard.old.database().close_admission();
        guard
            .old
            .database()
            .resolve_storage_recoveries()
            .map_err(|_| SessionRecoveryError::DatabaseClaim)?;
        match guard
            .old
            .database()
            .drain(&DatabaseSessionDrainControl::new(
                DatabaseDrainDeadline::at(deadline),
            )) {
            DatabaseDrainOutcome::Drained { .. } => {
                guard.finish(Some(SessionRecoveryPhase::ClearOldProject))
            }
            DatabaseDrainOutcome::TimedOut { .. } => {
                Err(SessionRecoveryError::DeadlineElapsed { phase })
            }
        }
    }

    #[cfg(test)]
    pub(super) fn install_recovery_for_test(
        &self,
        old: Arc<ApplicationSession>,
        epoch: ApplicationSessionEpoch,
        phase: SessionRecoveryPhase,
    ) -> RecoveryRequired {
        let recovery = SessionRecoveryId::from_existing(epoch.get());
        *self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner()) = SessionSlotState::Recovering {
            epoch,
            recovery,
            retained: RetainedSessionRecovery {
                old,
                work: RecoveryWorkState::Available(phase),
            },
        };
        RecoveryRequired {
            recovery,
            failed_epoch: epoch,
            phase,
        }
    }

    #[cfg(test)]
    pub(super) fn complete_recovery_phase_for_test(
        &self,
        recovery: SessionRecoveryId,
        next: Option<SessionRecoveryPhase>,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        self.claim_recovery(recovery)?.finish_for_test(next)
    }

    #[cfg(test)]
    pub(super) fn set_recovering_for_test(
        &self,
        epoch: ApplicationSessionEpoch,
        recovery: SessionRecoveryId,
        old: Arc<ApplicationSession>,
        phase: SessionRecoveryPhase,
    ) {
        *self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner()) = SessionSlotState::Recovering {
            epoch,
            recovery,
            retained: RetainedSessionRecovery {
                old,
                work: RecoveryWorkState::Available(phase),
            },
        };
    }
}
