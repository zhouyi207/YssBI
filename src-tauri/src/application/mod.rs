pub mod bayes;
#[cfg(test)]
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
#[cfg(test)]
pub mod graph_execution;
pub mod graph_mutation;
pub mod graph_open;
pub mod hypothesis;
pub mod pin_preview_generation;
pub mod project_lifecycle;
pub mod project_query;
pub mod project_watcher;
pub mod resource_mutation;
pub mod statistical_input;
pub mod variable_mutation;
pub mod worksheet;
pub(crate) mod worksheet_plot;

pub mod events;
