use crate::graph::{GraphId, NodeId};
use crate::schema::NodeInstanceDTO;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventNode {
    NodeCreated {
        graph_id: GraphId,
        node_id: NodeId,
        data: NodeInstanceDTO,
    },
    NodeDeleted {
        graph_id: GraphId,
        node_id: NodeId,
    },
    NodesUpdated {
        subgraph_id: String,
        // nodes: Vec<SerializedNode>,
    },
}