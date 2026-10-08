//! Provider-neutral contracts shared by YssBI automation hosts and adapters.

#![forbid(unsafe_code)]

mod agents;
mod assistant;
mod capabilities;
mod chart;
mod context;
mod database;
mod document;
mod gateway;
mod graph;
mod harness;
mod inspection;
mod knowledge;
mod knowledge_index;
mod knowledge_tools;
mod mind;
pub mod model;
mod models;
mod persistence;
mod resources;
mod results;
mod run_options;
mod statistics;
mod ui;
mod validation;

pub use agents::*;
pub use assistant::*;
pub use capabilities::*;
pub use chart::*;
pub use context::*;
pub use database::*;
pub use document::*;
pub use gateway::*;
pub use graph::*;
pub use harness::*;
pub use inspection::*;
pub use knowledge::*;
pub use knowledge_index::*;
pub use knowledge_tools::*;
pub use mind::*;
pub use models::*;
pub use persistence::*;
pub use resources::*;
pub use results::*;
pub use run_options::*;
pub use statistics::*;
pub use ui::*;
pub use validation::{
    MAX_CATALOG_QUERY_BYTES, MAX_CATALOG_RESULTS, MAX_LOCALE_BYTES, MAX_RESOURCE_ID_BYTES,
};

pub(crate) use graph::validate_graph_edit_operation;
pub(crate) use validation::validate_resource_id;

#[cfg(test)]
mod tests;
