//! Application use cases and desktop runtime initialization.

pub mod activity_panel;
pub mod automation;
pub mod catalog_query;
pub mod chart;
pub mod chart_plot;
pub mod database;
pub(crate) mod database_mutation;
pub(crate) mod database_session;
pub mod execution;
pub mod graph_compile;
pub mod graph_contracts;
pub mod graph_open;
pub mod harness;
mod ipc;
pub mod pin_preview_generation;
pub mod plugins;
pub mod project_change;
pub mod project_failure;
pub mod project_lifecycle;
pub mod project_query;
pub mod resource_mutation;
pub mod runtime;

pub use ipc::invoke_handler;
pub use runtime::initialize;

pub mod events;
