pub mod bayes;
pub mod catalog_compatibility;
pub mod catalog_query;
pub mod computation_settings;
pub mod database;
pub(crate) mod database_mutation;
pub mod database_schema;
pub(crate) mod database_session;
pub mod editor_projection;
pub mod execution;
pub mod graph_contracts;
pub mod graph_execution;
pub mod graph_mutation;
pub mod graph_open;
pub mod hypothesis;
pub mod pin_preview_generation;
pub mod project_lifecycle;
pub mod project_watcher;
pub mod statistical_input;

#[cfg(test)]
pub mod events;

#[cfg(test)]
pub(crate) mod worksheet_plot;
