//! Cross-authority use-case orchestration without transport or Tauri dependencies.

pub mod activity_panel;
pub mod automation;
pub mod catalog_query;
pub mod chart;
pub mod chart_plot;
pub mod database;
pub(crate) mod database_mutation;
pub(crate) mod database_session;
pub mod editor_projection;
pub mod execution;
pub mod graph_commit;
pub mod graph_compile;
pub mod graph_contracts;
pub mod graph_open;
pub mod hypothesis;
pub mod pin_preview_generation;
pub mod plugins;
pub mod project_change;
pub mod project_failure;
pub mod project_lifecycle;
pub mod project_query;
pub mod resource_mutation;
pub mod statistics;

pub mod events;
