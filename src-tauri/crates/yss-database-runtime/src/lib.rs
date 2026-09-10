//! Session-scoped database state, authority, physical routing, and typed query APIs.
//!
//! This crate owns the runtime that composes canonical database contracts with
//! the committed dataset store and DataFusion. Project publication, Application workflows, transport DTOs, and
//! Tauri delivery remain outside this boundary.

mod database_instance;
mod database_state;
pub mod error;
pub mod plot_query;
mod project_storage;
pub mod runtime;
pub mod session_api;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

use yss_database_contract::{
    DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
};

pub use database_instance::{DatabaseInstance, MAX_GET_DATAFRAME_ROWS};
pub use database_state::{DatabaseState, DatasetEdit};
pub use project_storage::{bind_dataset_instance, dataset_query_engine};

fn declaration_observation_for<'a>(
    observations: &'a DatabaseDeclarationObservationSet,
    database: &DatabaseId,
) -> Option<&'a DatabaseDeclarationObservation> {
    observations
        .iter()
        .find_map(|(id, observation)| (id == database).then_some(observation))
}

#[cfg(test)]
mod foundation_tests;
