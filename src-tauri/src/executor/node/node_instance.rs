//! Node 实例（运行时）
//!
//! Node 实例仅包含 ID 和对定义的引用，不持有 Pin 或状态。

use super::{NodeDefinition, NodeState};
use crate::executor::node::NodeId;
use std::sync::Arc;

/// Node 实例（运行时）
#[derive(Clone)]
pub struct NodeInstance {
    /// 节点 ID
    pub id: NodeId,

    /// 节点类型（用于查找定义）
    pub node_type: String,

    /// 子图 ID（用于 Subgraph 节点）
    pub sub_graph_id: Option<String>,

    /// 节点标题（可自定义）
    pub title: String,

    /// 节点定义引用
    pub definition: Arc<NodeDefinition>,

    /// 节点状态
    pub state: NodeState,

    /// UI 位置
    pub position: (f32, f32),

    /// 变量 ID（用于 Get/Set Variable 节点）
    pub variable_id: Option<String>,
}

impl std::fmt::Debug for NodeInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeInstance")
            .field("id", &self.id)
            .field("node_type", &self.node_type)
            .field("title", &self.title)
            .field("state", &self.state)
            .finish()
    }
}

impl NodeInstance {
    /// 从定义创建实例
    pub fn from_definition(definition: Arc<NodeDefinition>) -> Self {
        Self {
            id: NodeId::new(),
            node_type: definition.node_type.clone(),
            title: definition.title.clone(),
            definition: definition,
            state: NodeState::Idle,
            position: (0.0, 0.0),
            variable_id: None,
            sub_graph_id: None,
        }
    }

    /// 设置位置
    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.position = (x, y);
        self
    }

    /// 设置变量 ID
    pub fn with_variable(mut self, var_id: impl Into<String>) -> Self {
        self.variable_id = Some(var_id.into());
        self
    }

    /// 设置子图 ID
    pub fn with_subgraph(mut self, graph_id: impl Into<String>) -> Self {
        self.sub_graph_id = Some(graph_id.into());
        self
    }

    /// 获取定义
    pub fn definition(&self) -> &NodeDefinition {
        &self.definition
    }
}
