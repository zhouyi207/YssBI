//! Persisted database declaration and engine identity contracts.

mod declaration;
mod engine;

pub use declaration::DatabaseDecl;
pub use engine::{DatabaseEngine, DatabaseEngineSql};
