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
    NodeDeleted { graph_id: GraphId, node_id: NodeId },
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
    NodesUpdated { subgraph_id: String },
    /// 节点的动态 pins 发生变化（由 PinResolver 触发）
    #[serde(rename_all = "camelCase")]
    NodePinsUpdated {
        graph_id: GraphId,
        node_id: NodeId,
        /// 被移除的 pin IDs
        removed_pin_ids: Vec<PinId>,
        /// 新增的 pins（完整 DTO）
        added_pins: Vec<PinInstanceDTO>,
        /// 重命名/重索引后的 pins（完整 DTO）
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        updated_pins: Vec<PinInstanceDTO>,
        /// 被断开的连接 (from_pin, to_pin)
        removed_connections: Vec<(PinId, PinId)>,
        /// 变更后节点的完整 pin 顺序（用于前端重排）
        #[serde(skip_serializing_if = "Option::is_none")]
        pin_order: Option<Vec<PinId>>,
    },
    /// 类型推断后 pin 的解析类型发生变化
    #[serde(rename_all = "camelCase")]
    PinTypesInferred {
        graph_id: GraphId,
        pin_types: Vec<InferredPinType>,
    },
}

/// 单个 pin 的推断结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferredPinType {
    pub pin_id: PinId,
    /// 基础类型字符串（用于颜色），如 "float", "int", "string"
    pub pin_type: String,
    /// 容器类型（用于形状），如 "array", "dataseries"，基础类型为 None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_type: Option<String>,
    /// 完整类型描述字符串（用于 tooltip），如 "DataSeries<Float64 | String>"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_display: Option<String>,
}
