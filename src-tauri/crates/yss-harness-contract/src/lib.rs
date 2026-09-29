//! Provider-neutral contracts shared by YssBI automation hosts and adapters.

#![forbid(unsafe_code)]

mod agents;
mod capabilities;
mod context;
mod gateway;
mod graph;
mod harness;
mod inspection;
mod knowledge_memory;
mod persistence;
mod resources;
mod statistics;
mod validation;

pub use agents::*;
pub use capabilities::*;
pub use context::*;
pub use gateway::*;
pub use graph::*;
pub use harness::*;
pub use inspection::*;
pub use knowledge_memory::*;
pub use persistence::*;
pub use resources::*;
pub use statistics::*;
pub use validation::{
    MAX_CATALOG_QUERY_BYTES, MAX_CATALOG_RESULTS, MAX_LOCALE_BYTES, MAX_RESOURCE_ID_BYTES,
};

pub(crate) use graph::validate_graph_edit_operation;
pub(crate) use validation::validate_resource_id;

#[cfg(test)]
mod tests;
