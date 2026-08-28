use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use thiserror::Error;

use super::session_factory::UnpublishedApplicationSession;
use crate::database::runtime::DatabaseRuntimeSession;
use crate::execution::identity::{ExecutionSessionId, RuntimeGeneration};
use crate::execution::resource_preparation::ResourceProviderFactory;
use crate::execution::state::ExecutionRuntimeState;
use crate::graph::runtime_state::GraphRuntimeState;
use crate::node_system::ProjectSessionId;
use crate::project::{ProjectInstanceId, ProjectState};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ApplicationSessionEpoch(u64);

impl ApplicationSessionEpoch {
    pub const INITIAL: Self = Self(0);

    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

pub struct ApplicationSession {
    epoch: ApplicationSessionEpoch,
    project_instance_id: ProjectInstanceId,
    project_session_id: ProjectSessionId,
    execution_session_id: ExecutionSessionId,
    runtime_generation: RuntimeGeneration,
    project: Arc<ProjectState>,
    graph: Arc<GraphRuntimeState>,
    execution: Arc<ExecutionRuntimeState>,
    database: Arc<DatabaseRuntimeSession>,
    resource_provider_factory: Arc<ResourceProviderFactory>,
}

impl ApplicationSession {
    pub(super) fn from_candidate(
        epoch: ApplicationSessionEpoch,
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
        execution_session_id: ExecutionSessionId,
        runtime_generation: RuntimeGeneration,
        project: Arc<ProjectState>,
        graph: Arc<GraphRuntimeState>,
        execution: Arc<ExecutionRuntimeState>,
        database: Arc<DatabaseRuntimeSession>,
        resource_provider_factory: Arc<ResourceProviderFactory>,
    ) -> Self {
        Self {
            epoch,
            project_instance_id,
            project_session_id,
            execution_session_id,
            runtime_generation,
            project,
            graph,
            execution,
            database,
            resource_provider_factory,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        epoch: ApplicationSessionEpoch,
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
        execution_session_id: ExecutionSessionId,
        runtime_generation: RuntimeGeneration,
        project: Arc<ProjectState>,
        graph: Arc<GraphRuntimeState>,
        execution: Arc<ExecutionRuntimeState>,
        database: Arc<DatabaseRuntimeSession>,
        resource_provider_factory: Arc<ResourceProviderFactory>,
    ) -> Self {
        Self {
            epoch,
            project_instance_id,
            project_session_id,
            execution_session_id,
            runtime_generation,
            project,
            graph,
            execution,
            database,
            resource_provider_factory,
        }
    }

    pub(crate) fn project(&self) -> &ProjectState {
        &self.project
    }

    pub(crate) fn graph(&self) -> &GraphRuntimeState {
        &self.graph
    }

    pub(crate) fn execution(&self) -> &ExecutionRuntimeState {
        &self.execution
    }

    pub(crate) fn database(&self) -> &DatabaseRuntimeSession {
        &self.database
    }

    pub(crate) fn resource_provider_factory(&self) -> &ResourceProviderFactory {
        &self.resource_provider_factory
    }

    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn project_session_id(&self) -> &ProjectSessionId {
        &self.project_session_id
    }

    pub fn execution_session_id(&self) -> ExecutionSessionId {
        self.execution_session_id
    }

    pub fn runtime_generation(&self) -> RuntimeGeneration {
        self.runtime_generation
    }

    pub fn epoch(&self) -> ApplicationSessionEpoch {
        self.epoch
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SessionRecoveryId(u64);

impl SessionRecoveryId {
    const fn from_existing(value: u64) -> Self {
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
    #[error("session recovery phase is staged without an installed coordinator")]
    StagedOnly { phase: SessionRecoveryPhase },
    #[error("session recovery claim is no longer current")]
    StaleClaim,
    #[error("session recovery claim has already been consumed")]
    ClaimConsumed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
enum SessionReplacementError {
    #[error("application session is inactive")]
    Inactive,
    #[error("application session replacement is already in progress")]
    Replacing,
    #[error("application session recovery is required")]
    Recovering,
    #[error("application session epoch is exhausted")]
    EpochExhausted,
    #[error("application session replacement worker is stale")]
    StaleWorker,
    #[error("application session replacement phase is not current")]
    WrongPhase,
    #[error("candidate construction is not installed in the staged slot")]
    CandidateConstructionUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum SessionCaptureError {
    #[error("application session is inactive")]
    Inactive,
    #[error("application session replacement is in progress")]
    Replacing,
    #[error("application session recovery is required")]
    Recovering,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum SessionRevalidationError {
    #[error(transparent)]
    Unavailable(SessionCaptureError),
    #[error("captured application session changed")]
    Changed,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub(crate) enum SessionInstallationError {
    #[error("application session slot is not inactive")]
    SlotNotInactive,
    #[error("application session candidate epoch does not match the slot")]
    CandidateEpochMismatch,
}

struct SessionSlotInner {
    state: RwLock<SessionSlotState>,
    next_attempt: AtomicU64,
}

enum SessionSlotState {
    Inactive {
        next_epoch: ApplicationSessionEpoch,
    },
    Replacing {
        epoch: ApplicationSessionEpoch,
        phase: ReplacementPhase,
    },
    Recovering {
        epoch: ApplicationSessionEpoch,
        recovery: SessionRecoveryId,
        retained: RetainedSessionRecovery,
    },
    Active(Arc<ApplicationSession>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplacementPhase {
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
enum ReplacementAdvanceOutcome {
    Advanced(ReplacementPhase),
    Superseded(RecoveryRequired),
}

struct RetainedSessionRecovery {
    old: Arc<ApplicationSession>,
    work: RecoveryWorkState,
}

enum RecoveryWorkState {
    Available(SessionRecoveryPhase),
    InProgress {
        attempt: SessionRecoveryAttemptId,
        phase: SessionRecoveryPhase,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SessionRecoveryAttemptId(u64);

struct ReplacementWorker {
    slot: Arc<SessionSlotInner>,
    epoch: ApplicationSessionEpoch,
    phase: ReplacementPhase,
    old: Option<Arc<ApplicationSession>>,
    completed: bool,
}

struct SessionRecoveryClaimGuard {
    slot: Arc<SessionSlotInner>,
    epoch: ApplicationSessionEpoch,
    recovery: SessionRecoveryId,
    attempt: SessionRecoveryAttemptId,
    #[allow(
        dead_code,
        reason = "the guard retains this owner across off-lock work"
    )]
    old: Arc<ApplicationSession>,
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

impl ReplacementWorker {
    #[cfg(test)]
    fn complete_phase_for_test(
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

    #[allow(
        dead_code,
        reason = "replacement failure transfer is staged until the production coordinator exists"
    )]
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

impl SessionRecoveryClaimGuard {
    fn phase(&self) -> SessionRecoveryPhase {
        self.phase
    }

    #[cfg(test)]
    fn finish_for_test(
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

fn recovery_required(
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

// Task 3 intentionally stops at the ownership-safe boundary. It does not
// close/drain concrete runtimes, clear or hydrate Project state, build a
// candidate, or publish a production session. Those operations require the
// later composition builder and remain unreachable from the production root.
pub struct ApplicationSessionSlot {
    inner: Arc<SessionSlotInner>,
}

impl ApplicationSessionSlot {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SessionSlotInner {
                state: RwLock::new(SessionSlotState::Inactive {
                    next_epoch: ApplicationSessionEpoch::INITIAL,
                }),
                next_attempt: AtomicU64::new(1),
            }),
        }
    }

    fn install_candidate(
        &self,
        candidate: UnpublishedApplicationSession,
    ) -> Result<(), SessionInstallationError> {
        let mut state = self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let next_epoch = match &*state {
            SessionSlotState::Inactive { next_epoch } => *next_epoch,
            SessionSlotState::Replacing { .. }
            | SessionSlotState::Recovering { .. }
            | SessionSlotState::Active(_) => {
                return Err(SessionInstallationError::SlotNotInactive);
            }
        };

        let session = candidate.into_session();
        if session.epoch() != next_epoch {
            return Err(SessionInstallationError::CandidateEpochMismatch);
        }

        *state = SessionSlotState::Active(Arc::new(session));
        Ok(())
    }

    #[allow(
        dead_code,
        reason = "replacement orchestration is staged until the atomic production cutover"
    )]
    fn begin_replacement(&self) -> Result<ReplacementWorker, SessionReplacementError> {
        let mut state = self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let (old, epoch) = match &*state {
            SessionSlotState::Active(session) => {
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

    pub fn capture_session(&self) -> Result<Arc<ApplicationSession>, SessionCaptureError> {
        let state = self
            .inner
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        match &*state {
            SessionSlotState::Inactive { .. } => Err(SessionCaptureError::Inactive),
            SessionSlotState::Replacing { .. } => Err(SessionCaptureError::Replacing),
            SessionSlotState::Recovering { .. } => Err(SessionCaptureError::Recovering),
            SessionSlotState::Active(session) => Ok(Arc::clone(session)),
        }
    }

    pub fn revalidate_captured_session(
        &self,
        captured: &Arc<ApplicationSession>,
    ) -> Result<(), SessionRevalidationError> {
        let state = self
            .inner
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        match &*state {
            SessionSlotState::Inactive { .. } => Err(SessionRevalidationError::Unavailable(
                SessionCaptureError::Inactive,
            )),
            SessionSlotState::Replacing { .. } => Err(SessionRevalidationError::Unavailable(
                SessionCaptureError::Replacing,
            )),
            SessionSlotState::Recovering { .. } => Err(SessionRevalidationError::Unavailable(
                SessionCaptureError::Recovering,
            )),
            SessionSlotState::Active(current) if Arc::ptr_eq(current, captured) => Ok(()),
            SessionSlotState::Active(_) => Err(SessionRevalidationError::Changed),
        }
    }

    #[allow(
        dead_code,
        reason = "recovery orchestration is staged until Application owns replacement I/O"
    )]
    fn claim_recovery(
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

    #[allow(
        dead_code,
        reason = "the staged slot has no production replacement coordinator yet"
    )]
    fn retry_session_recovery(
        &self,
        recovery: SessionRecoveryId,
        control: &SessionRecoveryControl,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        let guard = self.claim_recovery(recovery)?;
        let phase = guard.phase();
        if control.deadline().is_expired() {
            return Err(SessionRecoveryError::DeadlineElapsed { phase });
        }
        // The slot owns the retained session and single-claimer protocol now.
        // Actual Execution/Database drain and authority resolution require the
        // composition builder introduced by the later production cutover.
        Err(SessionRecoveryError::StagedOnly { phase })
    }

    #[allow(
        dead_code,
        reason = "Database authority resolution is staged until its coordinator is installed"
    )]
    fn resolve_session_database_recovery(
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
        // Do not call the incomplete Database recovery seam or manufacture a
        // successful resolution. The guard drops and reinstalls this exact
        // retained work before returning the typed staged-only result.
        Err(SessionRecoveryError::StagedOnly { phase })
    }

    #[cfg(test)]
    fn begin_replacement_for_test(&self) -> Result<ReplacementWorker, SessionReplacementError> {
        self.begin_replacement()
    }

    #[cfg(test)]
    fn install_recovery_for_test(
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
    fn complete_recovery_phase_for_test(
        &self,
        recovery: SessionRecoveryId,
        next: Option<SessionRecoveryPhase>,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        self.claim_recovery(recovery)?.finish_for_test(next)
    }

    #[cfg(test)]
    pub(crate) fn publish_for_test(&self, session: Arc<ApplicationSession>) {
        let mut state = self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        *state = SessionSlotState::Active(session);
    }

    #[cfg(test)]
    fn set_replacing_for_test(&self, epoch: ApplicationSessionEpoch, phase: ReplacementPhase) {
        *self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner()) =
            SessionSlotState::Replacing { epoch, phase };
    }

    #[cfg(test)]
    fn set_recovering_for_test(
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

impl Default for ApplicationSessionSlot {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct ApplicationState {
    session_slot: Arc<ApplicationSessionSlot>,
}

impl ApplicationState {
    pub fn new(session_slot: Arc<ApplicationSessionSlot>) -> Self {
        Self { session_slot }
    }

    pub(crate) fn install_candidate(
        &self,
        candidate: UnpublishedApplicationSession,
    ) -> Result<(), SessionInstallationError> {
        self.session_slot.install_candidate(candidate)
    }

    pub fn capture_session(&self) -> Result<Arc<ApplicationSession>, SessionCaptureError> {
        self.session_slot.capture_session()
    }

    pub(crate) fn revalidate_captured_session(
        &self,
        captured: &Arc<ApplicationSession>,
    ) -> Result<(), SessionRevalidationError> {
        self.session_slot.revalidate_captured_session(captured)
    }

    pub fn retry_session_recovery(
        &self,
        recovery: SessionRecoveryId,
        control: &SessionRecoveryControl,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        self.session_slot.retry_session_recovery(recovery, control)
    }

    pub fn resolve_session_database_recovery(
        &self,
        recovery: SessionRecoveryId,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        self.session_slot
            .resolve_session_database_recovery(recovery)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::runtime::DatabaseRuntimeRegistry;
    use crate::database_contract::{
        DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet,
        DatabaseId, DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };
    use crate::execution::identity::ExecutionSessionId;
    use crate::graph::resource_catalog::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
    use crate::graph::runtime_state::{GraphRuntimeComponents, GraphRuntimeEpoch};
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::compiler::ProjectCompileCoordinator;
    use std::collections::BTreeMap;
    use std::num::NonZeroU64;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{Duration, Instant};

    fn session(epoch: u64) -> Arc<ApplicationSession> {
        let project_session_id = ProjectSessionId::new(format!("session-{epoch}"));
        let execution_session_id = ExecutionSessionId::new(uuid::Uuid::from_u128(epoch as u128));
        let project = Arc::new(ProjectState::new());
        let builtin = build_builtin_node_system().expect("test built-ins are valid");
        let graph = Arc::new(GraphRuntimeState::from_components(
            GraphRuntimeEpoch::from_existing(epoch),
            GraphRuntimeComponents {
                registry: builtin.registry,
                catalog: builtin.catalog,
                compiler: Arc::new(ProjectCompileCoordinator::new()),
                resource_catalog: Arc::new(ResourceCatalogSnapshot::new(
                    BTreeMap::new(),
                    BTreeMap::new(),
                    BTreeMap::new(),
                    ResourceCatalogFingerprint::from_bytes([epoch as u8; 32]),
                )),
            },
        ));
        let observations = DatabaseDeclarationObservationSet::try_from_iter(std::iter::empty::<(
            DatabaseId,
            DatabaseDeclarationObservation,
        )>())
        .expect("empty observation set is valid");
        let declarations: Arc<[DatabaseDecl]> = Vec::new().into();
        let database = Arc::new(
            DatabaseRuntimeRegistry::new()
                .open_session(DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing(project_session_id.as_str().into()),
                    NonZeroU64::new(1).expect("non-zero test generation"),
                    None,
                    declarations,
                    observations,
                ))
                .expect("empty database session is valid"),
        );
        let execution = Arc::new(ExecutionRuntimeState::new(
            execution_session_id,
            RuntimeGeneration::from_existing(epoch),
        ));
        let resource_provider_factory = Arc::new(ResourceProviderFactory::new(
            project_session_id.as_str().into(),
        ));
        Arc::new(ApplicationSession::new_for_test(
            ApplicationSessionEpoch::from_existing(epoch),
            ProjectInstanceId::from_existing(format!("project-{epoch}")),
            project_session_id,
            execution_session_id,
            RuntimeGeneration::from_existing(epoch),
            project,
            graph,
            execution,
            database,
            resource_provider_factory,
        ))
    }

    #[test]
    fn capture_and_revalidate_use_one_session_envelope() {
        let slot = ApplicationSessionSlot::new();
        assert!(matches!(
            slot.capture_session(),
            Err(SessionCaptureError::Inactive)
        ));

        let first = session(1);
        slot.publish_for_test(Arc::clone(&first));
        let captured = slot
            .capture_session()
            .expect("active session is capturable");
        assert!(Arc::ptr_eq(&captured, &first));
        assert_eq!(captured.project_session_id().as_str(), "session-1");
        assert_eq!(captured.graph().epoch().get(), 1);
        assert_eq!(
            captured.execution_session_id(),
            captured.execution().session_id()
        );
        assert_eq!(
            captured.runtime_generation(),
            captured.execution().generation()
        );
        assert!(slot.revalidate_captured_session(&captured).is_ok());

        let second = session(2);
        slot.publish_for_test(Arc::clone(&second));
        assert!(matches!(
            slot.revalidate_captured_session(&captured),
            Err(SessionRevalidationError::Changed)
        ));
    }

    #[test]
    fn stale_replacement_worker_stops_after_recovery_supersedes_its_phase() {
        let slot = ApplicationSessionSlot::new();
        let old = session(1);
        slot.publish_for_test(Arc::clone(&old));
        let mut worker = slot
            .begin_replacement_for_test()
            .expect("active slot replaces");
        let required = slot.install_recovery_for_test(
            Arc::clone(&old),
            worker.epoch,
            SessionRecoveryPhase::DrainOldExecution,
        );

        assert_eq!(
            worker
                .complete_phase_for_test(ReplacementPhase::CloseAdmissions)
                .expect("superseded worker reports retained recovery"),
            ReplacementAdvanceOutcome::Superseded(required)
        );
        assert!(matches!(
            slot.capture_session(),
            Err(SessionCaptureError::Recovering)
        ));
        let state = slot
            .inner
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        assert!(matches!(
            &*state,
            SessionSlotState::Recovering {
                epoch,
                recovery,
                retained: RetainedSessionRecovery {
                    old: retained,
                    work: RecoveryWorkState::Available(SessionRecoveryPhase::DrainOldExecution),
                },
            } if *epoch == worker.epoch
                && *recovery == required.recovery
                && Arc::ptr_eq(retained, &old)
        ));
    }

    #[test]
    fn recovery_claim_is_single_owner_and_drop_reinstalls_exact_work() {
        let slot = Arc::new(ApplicationSessionSlot::new());
        let old = session(1);
        let required = slot.install_recovery_for_test(
            Arc::clone(&old),
            ApplicationSessionEpoch::from_existing(2),
            SessionRecoveryPhase::ResolveOldDatabase,
        );
        let application = ApplicationState::new(Arc::clone(&slot));
        let control = SessionRecoveryControl::new(SessionRecoveryDeadline::at(
            Instant::now() + Duration::from_secs(1),
        ));
        assert!(matches!(
            application.retry_session_recovery(required.recovery, &control),
            Err(SessionRecoveryError::StagedOnly {
                phase: SessionRecoveryPhase::ResolveOldDatabase,
            })
        ));
        let first = slot
            .claim_recovery(required.recovery)
            .expect("first recovery claimant owns the work");
        let barrier = Arc::new(Barrier::new(2));
        let concurrent_slot = Arc::clone(&slot);
        let concurrent_barrier = Arc::clone(&barrier);
        let recovery = required.recovery;
        let second = thread::spawn(move || {
            concurrent_barrier.wait();
            concurrent_slot.claim_recovery(recovery)
        });
        barrier.wait();
        assert!(matches!(
            second.join().expect("claim thread completes"),
            Err(SessionRecoveryError::AlreadyInProgress)
        ));
        drop(first);

        let state = slot
            .inner
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        assert!(matches!(
            &*state,
            SessionSlotState::Recovering {
                recovery: current,
                retained: RetainedSessionRecovery {
                    old: retained,
                    work: RecoveryWorkState::Available(SessionRecoveryPhase::ResolveOldDatabase),
                },
                ..
            } if *current == recovery && Arc::ptr_eq(retained, &old)
        ));
        drop(state);

        let second_claim = slot
            .claim_recovery(recovery)
            .expect("reinstalled recovery can be claimed again");
        assert!(Arc::ptr_eq(&second_claim.old, &old));
        assert_eq!(
            second_claim.phase(),
            SessionRecoveryPhase::ResolveOldDatabase
        );
        drop(second_claim);

        assert_eq!(
            slot.complete_recovery_phase_for_test(
                recovery,
                Some(SessionRecoveryPhase::DrainOldDatabase),
            )
            .expect("recovery advances with the same retained owner"),
            SessionRecoveryOutcome::RetryRequired(RecoveryRequired {
                recovery,
                failed_epoch: ApplicationSessionEpoch::from_existing(2),
                phase: SessionRecoveryPhase::DrainOldDatabase,
            })
        );
        let terminal = slot
            .complete_recovery_phase_for_test(recovery, None)
            .expect("terminal recovery consumes the retained owner");
        assert_eq!(
            terminal,
            SessionRecoveryOutcome::ReplacementMayRestart {
                next_epoch: ApplicationSessionEpoch::from_existing(3),
            }
        );
        assert!(matches!(
            slot.capture_session(),
            Err(SessionCaptureError::Inactive)
        ));
    }

    #[test]
    fn non_active_capture_errors_remain_fieldless() {
        let slot = ApplicationSessionSlot::new();
        slot.set_replacing_for_test(
            ApplicationSessionEpoch::from_existing(1),
            ReplacementPhase::DrainDatabase,
        );
        assert!(matches!(
            slot.capture_session(),
            Err(SessionCaptureError::Replacing)
        ));
        slot.set_recovering_for_test(
            ApplicationSessionEpoch::from_existing(1),
            SessionRecoveryId::from_existing(1),
            session(1),
            SessionRecoveryPhase::ClearOldProject,
        );
        assert!(matches!(
            slot.capture_session(),
            Err(SessionCaptureError::Recovering)
        ));
    }
}
