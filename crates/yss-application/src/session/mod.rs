//! Complete application sessions, independent of any one business component.

mod application_session;
mod components;
mod database;
mod factory;
mod slot;

pub use application_session::{ApplicationSession, ApplicationSessionEpoch};
pub use components::{NodeComponents, NodeCompositionError};

pub use factory::{
    ApplicationInitializationError, DatabaseSessionCandidateError, InvalidSessionCandidateError,
    ProjectSessionCandidateError, UnpublishedApplicationSession, build_current_project_candidate,
};
pub(crate) use slot::ProjectReplacement;
pub use slot::{
    ApplicationSessionRefreshError, ApplicationSessionSlot, ApplicationState, RecoveryRequired,
    SessionCaptureError, SessionInstallationError, SessionRecoveryControl, SessionRecoveryDeadline,
    SessionRecoveryError, SessionRecoveryId, SessionRecoveryOutcome, SessionRecoveryPhase,
    SessionRevalidationError,
};
