//! Session-scoped database state, authority, physical routing, and typed query APIs.
//!
//! This crate owns the runtime that composes canonical database contracts with
//! the committed dataset store and DataFusion. Project owns resource publication;
//! Application coordinates workflows and the native host consumes typed snapshots.

mod database_instance;
mod database_state;
mod edit_history;
pub mod error;
pub mod plot_query;
mod profile_query;
mod project_storage;
pub mod runtime;
pub mod session_api;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use database_instance::{DatabaseInstance, MAX_GET_DATAFRAME_ROWS};
pub(crate) use database_state::{DatabaseState, DatasetEdit};
pub use project_storage::{bind_dataset_instance, dataset_query_engine};
pub use yss_database_engine::DatasetRowsQuery;

#[cfg(test)]
mod foundation_tests;
