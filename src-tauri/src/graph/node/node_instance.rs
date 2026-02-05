//! Node 实例（运行时）
//!
//! Node 实例仅包含 ID 和对定义的引用，不持有 Pin 或状态。
//!
//! 🧱 第三层：Role → PinId 映射
//!
//! 对于动态 Pin，映射关系存储在 NodeInstance 中。
//! 静态 Pin 的映射通过 Graph 查询 PinDefinition 完成。

use super::{NodeDefinition, NodeState};
use crate::graph::node::NodeId;
use crate::graph::pin::{PinId, PinRole};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Node 实例（运行时）
#[derive(Clone, Deserialize, Serialize)]
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

    /// 🧱 第三层：动态 Pin 的 Role → PinId 映射
    ///
    /// 静态 Pin 不需要存储映射（通过 PinDefinition.role 查询）
    /// 动态 Pin 在运行时添加，需要记录其 Role 映射
    ///
    /// 例如：Sequence 节点动态添加第 3 个输出时：
    /// - PinRole::Exec(ExecRole::Steps(2)) -> pin_id_xxx
    pub role_to_pin: Arc<RwLock<HashMap<PinRole, PinId>>>,
}

impl std::fmt::Debug for NodeInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeInstance")
            .field("id", &self.id)
            .field("node_type", &self.node_type)
            .field("title", &self.title)
            .field("state", &self.state)
            .field("dynamic_pins", &self.role_to_pin.read().unwrap().len())
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
            role_to_pin: Arc::new(RwLock::new(HashMap::new())),
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

    // =========================
    // 🧱 第三层：Role → PinId 映射管理
    // =========================

    /// 注册动态 Pin 的 Role 映射
    pub fn register_dynamic_pin(&self, role: PinRole, pin_id: PinId) {
        self.role_to_pin.write().unwrap().insert(role, pin_id);
    }

    /// 移除动态 Pin 的 Role 映射
    pub fn unregister_dynamic_pin(&self, role: &PinRole) -> Option<PinId> {
        self.role_to_pin.write().unwrap().remove(role)
    }

    /// 查询动态 Pin 的 PinId
    pub fn get_dynamic_pin_id(&self, role: &PinRole) -> Option<PinId> {
        self.role_to_pin.read().unwrap().get(role).copied()
    }

    /// 获取所有动态 Pin 映射
    pub fn get_all_dynamic_mappings(&self) -> HashMap<PinRole, PinId> {
        self.role_to_pin.read().unwrap().clone()
    }

    /// 检查是否有动态 Pin
    pub fn has_dynamic_pins(&self) -> bool {
        !self.role_to_pin.read().unwrap().is_empty()
    }

    /// 清空所有动态 Pin 映射
    pub fn clear_dynamic_pins(&self) {
        self.role_to_pin.write().unwrap().clear();
    }
}
