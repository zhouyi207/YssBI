use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

use thiserror::Error;

use super::factory::UnpublishedApplicationSession;
use super::{ApplicationSession, ApplicationSessionEpoch};

mod recovery;
mod replacement;

use recovery::RetainedSessionRecovery;
pub use recovery::{
    RecoveryRequired, SessionRecoveryControl, SessionRecoveryDeadline, SessionRecoveryError,
    SessionRecoveryId, SessionRecoveryOutcome, SessionRecoveryPhase,
};
pub(crate) use replacement::ProjectReplacement;
use replacement::{ReplacementPhase, SessionReplacementError};

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
pub enum SessionInstallationError {
    #[error(
        "candidate node definitions or execution capabilities differ from the session configuration"
    )]
    CandidateComponentsMismatch,
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

// The slot is the sole production owner of session replacement. All fallible
// drain/build work happens outside its short state lock and publishes one
// complete Project/Graph/Execution/Database envelope.
pub struct ApplicationSessionSlot {
    nodes: super::NodeComponents,
    inner: Arc<SessionSlotInner>,
}

impl ApplicationSessionSlot {
    pub fn new(nodes: super::NodeComponents) -> Self {
        Self {
            nodes,
            inner: Arc::new(SessionSlotInner {
                state: RwLock::new(SessionSlotState::Inactive {
                    next_epoch: ApplicationSessionEpoch::INITIAL,
                }),
                next_attempt: AtomicU64::new(1),
            }),
        }
    }

    pub(crate) fn install_candidate(
        &self,
        candidate: UnpublishedApplicationSession,
    ) -> Result<(), SessionInstallationError> {
        let session = candidate.into_session();
        let components_match = self.nodes.matches(&session);
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

        if !components_match {
            return Err(SessionInstallationError::CandidateComponentsMismatch);
        }
        if session.epoch() != next_epoch {
            return Err(SessionInstallationError::CandidateEpochMismatch);
        }

        *state = SessionSlotState::Active(Arc::new(session));
        Ok(())
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

    #[cfg(any(test, feature = "test-support"))]
    pub fn publish_for_test(&self, session: Arc<ApplicationSession>) {
        let mut state = self
            .inner
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        *state = SessionSlotState::Active(session);
    }
}

#[derive(Debug, Error)]
pub enum ApplicationSessionRefreshError {
    #[error("application session capture failed")]
    Capture(SessionCaptureError),
    #[error("application session candidate build failed")]
    Candidate,
    #[error("application session replacement failed")]
    Replacement,
}

#[derive(Clone)]
pub struct ApplicationState {
    session_slot: Arc<ApplicationSessionSlot>,
}

impl ApplicationState {
    pub fn new(session_slot: Arc<ApplicationSessionSlot>) -> Self {
        Self { session_slot }
    }

    pub fn install_candidate(
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

    pub(crate) fn begin_project_replacement(
        &self,
        captured: &Arc<ApplicationSession>,
    ) -> Result<ProjectReplacement, SessionReplacementError> {
        self.session_slot.begin_project_replacement(captured)
    }

    pub(crate) fn finish_project_replacement(
        &self,
        replacement: ProjectReplacement,
    ) -> Result<(), ApplicationSessionRefreshError> {
        self.session_slot.finish_project_replacement(replacement)
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

    pub(crate) fn rebuild_application_session(
        &self,
        captured: &Arc<ApplicationSession>,
    ) -> Result<(), ApplicationSessionRefreshError> {
        let replacement = self
            .begin_project_replacement(captured)
            .map_err(|_| ApplicationSessionRefreshError::Replacement)?;
        self.finish_project_replacement(replacement)
    }
}

#[cfg(test)]
mod tests;
