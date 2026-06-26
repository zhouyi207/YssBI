use crate::graph::{GraphId, PinId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventConnection {
    /// 连接已创建（from_pin → to_pin）
    #[serde(rename_all = "camelCase")]
    ConnectionCreated {
        graph_id: GraphId,
        from_pin: PinId,
        to_pin: PinId,
    },
    /// 连接已删除（from_pin → to_pin）
    #[serde(rename_all = "camelCase")]
    ConnectionDeleted {
        graph_id: GraphId,
        from_pin: PinId,
        to_pin: PinId,
    },
    /// Pin 的所有连接已断开（列出所有被删除的连接对）
    #[serde(rename_all = "camelCase")]
    ConnectionsBatchDeleted {
        graph_id: GraphId,
        removed_connections: Vec<(PinId, PinId)>,
    },
    /// 一批连接已创建（批量粘贴/恢复时一次性建立，前端单次入库）
    #[serde(rename_all = "camelCase")]
    ConnectionsBatchCreated {
        graph_id: GraphId,
        connections: Vec<(PinId, PinId)>,
    },
}
