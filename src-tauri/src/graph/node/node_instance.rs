//! Node 实例（运行时）

use super::{NodeDefinition, NodePosition};
use crate::graph::node::NodeId;
use crate::graph::{TypeVarId, PinId};
use crate::graph::TypeVarDefinition;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Node 实例（运行时）
#[derive(Clone, Deserialize, Serialize)]
pub struct NodeInstance {
    /// 节点 ID
    pub id: NodeId,

    /// 节点定义引用
    pub definition: Arc<NodeDefinition>,

    /// type_var 定义映射 (需要映射 pininstance 的 pindefinition 中的 data type)
    pub type_var_map: HashMap<TypeVarId, TypeVarDefinition>,

    /// UI 位置
    pub position: NodePosition,

    // pins
    pub pins: Vec<PinId>,
}

impl NodeInstance {
    /// 从定义创建实例
    pub fn from_definition(definition: Arc<NodeDefinition>) -> Self {
        todo!("在这里还需要处理 PinInstance 的注册, 如果有 type_var_id 需要给 PinInstance");
        // Self {
        //     id: NodeId::new(),
        //     definition,
        //     position: NodePosition::default(),
        //     // 在这里应该调用 definition 的 pin_generator 的逻辑生成 pins
        //     pins: vec![],
        // }
    }

    /// 设置位置
    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.position = NodePosition { x, y };
        self
    }
}
