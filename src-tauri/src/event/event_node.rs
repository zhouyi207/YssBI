use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventNode {
        NodesUpdated {
        subgraph_id: String,
        // nodes: Vec<SerializedNode>,
    },
}