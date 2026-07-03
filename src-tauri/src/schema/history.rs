//! History / Graph Rebuild DTOs
//!
//! 用于 undo/redo 时从前端快照重建后端 Graph 状态。

use crate::graph::node::{NodeInstanceParams, NodePosition};
use crate::graph::pin::PinInstance;
use crate::graph::{NodeId, TypeVarDefinition, TypeVarId};
use crate::graph::value::DataValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// 子图快照：对齐磁盘节点格式（完整 PinInstance，保留 ID），用于结构性 undo。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSubgraphDTO {
    pub id: NodeId,
    pub node_type: String,
    pub position: NodePosition,
    #[serde(default)]
    pub type_var_map: HashMap<TypeVarId, TypeVarDefinition>,
    #[serde(default)]
    pub instance_params: NodeInstanceParams,
    pub pins: Vec<PinInstance>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphUndoPatch {
    pub nodes: Vec<NodeSubgraphDTO>,
    #[serde(default)]
    pub neighbor_nodes: Vec<NodeSubgraphDTO>,
    pub connections: Vec<ConnectionRebuildDTO>,
}
