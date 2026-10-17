//! YssBI Assistant business contracts: typed tools, task authority and Assistant read projections.

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
mod inspection;
mod ledger;
mod mind;
pub mod model;
mod resources;
mod results;
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
pub use inspection::*;
pub use ledger::*;
pub use mind::*;
pub use resources::*;
pub use results::*;
pub use statistics::*;
pub use ui::*;
pub use validation::{MAX_CATALOG_QUERY_BYTES, MAX_CATALOG_RESULTS, MAX_LOCALE_BYTES};

pub(crate) use graph::validate_graph_edit_operation;
pub(crate) use yss_harness_contract::*;

#[cfg(test)]
mod tests;
