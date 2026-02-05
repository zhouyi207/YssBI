pub use crate::graph::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventConnection {
    ConnectionsUpdated {
        subgraph_id: String,
        connections: Vec<Connection>,
    },
}
