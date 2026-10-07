//! Canonical database declaration, identity, session, and operation contracts.

mod declaration;
mod edit_state;
mod engine;
mod export;
mod fingerprint;
mod identity;
mod observation;
mod session;

pub use declaration::DatabaseDecl;
pub use edit_state::EditState;
pub use engine::{DatabaseEngine, DatabaseEngineSql, DatabaseImportSource};
pub use export::{DatabaseExportFormat, DatabaseExportFormatParseError};
pub use identity::{DatabaseDeclarationFingerprint, DatabaseDeclarationRevision, DatabaseId};
pub use observation::{
    DatabaseDeclarationObservation, DatabaseDeclarationObservationSet,
    DatabaseDeclarationObservationSetError,
};
pub use session::{
    DatabaseSessionIdentity, DatabaseSessionOpenRequest, DatabaseSessionOpenRequestError,
    DatabaseSessionOpenRequestParts,
};
