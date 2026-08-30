//! Persisted database declaration and engine identity contracts.

mod declaration;
mod engine;
mod fingerprint;
mod identity;
mod observation;
mod session;

pub use declaration::DatabaseDecl;
pub use engine::{DatabaseEngine, DatabaseEngineSql};
pub use identity::{DatabaseDeclarationFingerprint, DatabaseDeclarationRevision, DatabaseId};
pub use observation::{
    DatabaseDeclarationObservation, DatabaseDeclarationObservationSet,
    DatabaseDeclarationObservationSetError,
};
pub use session::{
    DatabaseSessionIdentity, DatabaseSessionOpenRequest, DatabaseSessionOpenRequestError,
    DatabaseSessionOpenRequestParts,
};
