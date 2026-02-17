pub use crate::graph::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventConnection {
    #[serde(rename_all = "camelCase")]
    ConnectionsUpdated {
        subgraph_id: String,
        connections: Vec<Connection>,
    },
}
