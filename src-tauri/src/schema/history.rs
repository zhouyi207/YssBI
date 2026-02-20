//! History / Graph Rebuild DTOs
//!
//! 用于 undo/redo 时从前端快照重建后端 Graph 状态。

use crate::graph::node::NodeInstanceParams;
use crate::graph::value::DataValue;
use serde::{Deserialize, Serialize};

/// 单个节点的重建数据
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRebuildDTO {
    pub id: String,
    pub node_type: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub params: Option<NodeInstanceParams>,
    #[serde(default)]
    pub pins: Vec<PinRebuildDTO>,
}

/// 单个 Pin 的重建数据（保留 ID 和用户输入值）
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinRebuildDTO {
    pub id: String,
    #[serde(default)]
    pub user_value: Option<DataValue>,
}

/// 连接的重建数据
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRebuildDTO {
    pub from_pin: String,
    pub to_pin: String,
}

/// 完整的图重建快照
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRebuildSnapshot {
    pub nodes: Vec<NodeRebuildDTO>,
    pub connections: Vec<ConnectionRebuildDTO>,
}
