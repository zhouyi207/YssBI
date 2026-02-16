use crate::graph::{GraphId, NodeId};
use crate::schema::{NodeInstanceDTO, PinInstanceDTO};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventNode {
    NodeCreated {
        graph_id: GraphId,
        node_id: NodeId,
        data: NodeInstanceDTO,
        /// 新建节点的 pins，便于前端直接 hydrate 到 Store
        pins: Vec<PinInstanceDTO>,
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