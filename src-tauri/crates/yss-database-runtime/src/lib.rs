//! Session-scoped database state, authority, physical routing, and typed query APIs.
//!
//! This crate owns the cross-engine runtime that composes canonical database contracts with
//! Polars and DuckDB adapters. Project publication, Application workflows, transport DTOs, and
//! Tauri delivery remain outside this boundary.

mod database_instance;
mod database_state;
pub mod error;
pub mod plot_query;
mod project_storage;
pub mod runtime;
pub mod session_api;

use yss_database_contract::{
    DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
};

pub use database_instance::DatabaseInstance;
pub use database_state::DatabaseState;
pub use project_storage::{bind_duckdb_instance, remove_duckdb_table_if_needed};

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
