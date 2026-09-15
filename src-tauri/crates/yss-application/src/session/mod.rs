//! Complete application sessions, independent of any one business component.

mod components;
mod database;
mod factory;
mod slot;

pub use components::{NodeComponents, NodeCompositionError};

pub use factory::{
    ApplicationInitializationError, DatabaseSessionCandidateError, InvalidSessionCandidateError,
    ProjectSessionCandidateError, UnpublishedApplicationSession, build_current_project_candidate,
};
pub(crate) use slot::ProjectReplacement;
pub use slot::{
    ApplicationSession, ApplicationSessionEpoch, ApplicationSessionRefreshError,
    ApplicationSessionSlot, ApplicationState, RecoveryRequired, SessionCaptureError,
    SessionInstallationError, SessionRecoveryControl, SessionRecoveryDeadline,
    SessionRecoveryError, SessionRecoveryId, SessionRecoveryOutcome, SessionRecoveryPhase,
    SessionRevalidationError,
};
