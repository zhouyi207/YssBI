use std::sync::{Arc, RwLock};
use std::time::Instant;

use thiserror::Error;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
}

pub struct SessionRecoveryControl {
    deadline: SessionRecoveryDeadline,
}

impl SessionRecoveryControl {
    pub const fn new(deadline: SessionRecoveryDeadline) -> Self {
        Self { deadline }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryRequired {
    pub recovery: SessionRecoveryId,
    pub failed_epoch: ApplicationSessionEpoch,
    pub phase: SessionRecoveryPhase,
}

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

struct SessionSlotInner {
    state: RwLock<SessionSlotState>,
}

enum SessionSlotState {
    Inactive {
        next_epoch: ApplicationSessionEpoch,
    },
    Replacing {
        epoch: ApplicationSessionEpoch,
    },
    Recovering {
        epoch: ApplicationSessionEpoch,
        recovery: SessionRecoveryId,
    },
    Active(Arc<ApplicationSession>),
}

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
            }),
        }
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
    pub(crate) fn set_replacing_for_test(&self, epoch: ApplicationSessionEpoch) {
        *self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner()) = SessionSlotState::Replacing { epoch };
    }

    #[cfg(test)]
    pub(crate) fn set_recovering_for_test(
        &self,
        epoch: ApplicationSessionEpoch,
        recovery: SessionRecoveryId,
    ) {
        *self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner()) =
            SessionSlotState::Recovering { epoch, recovery };
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
        _recovery: SessionRecoveryId,
        _control: &SessionRecoveryControl,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        Err(SessionRecoveryError::NotFound)
    }

    pub fn resolve_session_database_recovery(
        &self,
        _recovery: SessionRecoveryId,
    ) -> Result<SessionRecoveryOutcome, SessionRecoveryError> {
        Err(SessionRecoveryError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_slot_reports_fieldless_capture_error() {
        let slot = ApplicationSessionSlot::new();
        assert!(matches!(
            slot.capture_session(),
            Err(SessionCaptureError::Inactive)
        ));
        slot.set_replacing_for_test(ApplicationSessionEpoch::from_existing(1));
        assert!(matches!(
            slot.capture_session(),
            Err(SessionCaptureError::Replacing)
        ));
        slot.set_recovering_for_test(
            ApplicationSessionEpoch::from_existing(1),
            SessionRecoveryId(1),
        );
        assert!(matches!(
            slot.capture_session(),
            Err(SessionCaptureError::Recovering)
        ));
    }
}
