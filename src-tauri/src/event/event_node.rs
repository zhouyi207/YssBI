use crate::graph::{GraphId, NodeId, PinId};
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
    NodesBatchDeleted {
        graph_id: GraphId,
        node_ids: Vec<NodeId>,
    },
    #[serde(rename_all = "camelCase")]
    NodePositionsUpdated {
        graph_id: GraphId,
        updates: Vec<(NodeId, f32, f32)>,
    },
    #[serde(rename_all = "camelCase")]
    NodesBatchCreated {
        graph_id: GraphId,
        nodes: Vec<(NodeId, NodeInstanceDTO, Vec<PinInstanceDTO>)>,
    },
    #[serde(rename_all = "camelCase")]
    NodesUpdated {
        subgraph_id: String,
    },
    /// 节点的动态 pins 发生变化（由 PinResolver 触发）
    #[serde(rename_all = "camelCase")]
    NodePinsUpdated {
        graph_id: GraphId,
        node_id: NodeId,
        /// 被移除的 pin IDs
        removed_pin_ids: Vec<PinId>,
        /// 新增的 pins（完整 DTO）
        added_pins: Vec<PinInstanceDTO>,
        /// 被断开的连接 (from_pin, to_pin)
        removed_connections: Vec<(PinId, PinId)>,
    },
    /// 类型推断后 pin 的解析类型发生变化
    #[serde(rename_all = "camelCase")]
    PinTypesInferred {
        graph_id: GraphId,
        /// (pin_id, resolved_type_string) — type string 与 PinInstanceDTO.pin_type 格式一致
        pin_types: Vec<(PinId, String)>,
    },
}