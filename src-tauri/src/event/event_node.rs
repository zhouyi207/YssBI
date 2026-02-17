use crate::graph::{GraphId, NodeId};
use crate::schema::{NodeInstanceDTO, PinInstanceDTO};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventNode {
    #[serde(rename_all = "camelCase")]
    NodeCreated {
        graph_id: GraphId,
        node_id: NodeId,
        data: NodeInstanceDTO,
        pins: Vec<PinInstanceDTO>,
    },
    #[serde(rename_all = "camelCase")]
    NodeDeleted {
        graph_id: GraphId,
        node_id: NodeId,
    },
    #[serde(rename_all = "camelCase")]
    NodePositionsUpdated {
        graph_id: GraphId,
        updates: Vec<(NodeId, f32, f32)>,
    },
    #[serde(rename_all = "camelCase")]
    NodesUpdated {
        subgraph_id: String,
    },
}