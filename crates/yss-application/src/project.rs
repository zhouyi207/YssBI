//! Project use cases and process-wide registration.

pub mod change;
pub(crate) mod failure;
pub mod lifecycle;
pub mod query;
mod registry;

pub use failure::ApplicationProjectFailure;
pub use registry::ProjectManagement;
