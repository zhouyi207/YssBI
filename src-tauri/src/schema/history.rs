//! Structural undo/redo patch DTOs.

use crate::graph::node::{NodeInstanceParams, NodePosition};
use crate::graph::pin::PinInstance;
use crate::graph::{NodeId, TypeVarDefinition, TypeVarId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 连接的重建数据
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRebuildDTO {
    pub from_pin: String,
    pub to_pin: String,
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
