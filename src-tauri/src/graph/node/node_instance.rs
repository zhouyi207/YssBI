//! Node 实例（运行时）

use super::{NodeDefinition, NodePosition};
use crate::graph::node::NodeId;
use crate::graph::TypeVarDefinition;
use crate::graph::{PinId, PinInstance, TypeVarId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 节点创建结果，便于扩展（如未来加入默认连接等）
#[derive(Debug)]
pub struct NodeCreationResult {
    pub node: NodeInstance,
    pub pins: Vec<PinInstance>,
}

/// Node 实例（运行时）
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
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
    pub pin_ids: Vec<PinId>,
}

impl NodeInstance {
    /// 从定义创建实例
    pub fn from_definition(definition: Arc<NodeDefinition>) -> Result<NodeCreationResult, String> {
        let node_id = NodeId::new();

        // ---------- TypeVar 映射 ----------
        let mut type_var_map = HashMap::new();
        let mut type_var_map_reverse = HashMap::new();

        for type_var in definition.type_vars.iter().cloned() {
            let type_var_id = TypeVarId::new();
            type_var_map.insert(type_var_id, type_var.clone());
            type_var_map_reverse.insert(type_var, type_var_id);
        }

        // ---------- 创建 PinInstance ----------
        let pin_defs = (definition
            .pin_generator
            .as_ref()
            .ok_or("pin_generator missing")?)()?;

        let pin_instances: Vec<PinInstance> = pin_defs
            .iter()
            .enumerate()
            .map(|(index, pin_definition)| {
                let order = index as i32;

                let mut pin = PinInstance::from_definition(pin_definition, node_id, order);

                // 如果 pin 使用了类型变量，设置 type_var_id
                if let Some(type_var_key) = pin_definition.get_type_var_key() {
                    // 从 NodeDefinition.type_vars 中找到对应的 TypeVarDefinition
                    if let Some(type_var_def) = definition.type_vars.iter().find(|tv| tv.id == type_var_key) {
                        // 从 type_var_map_reverse 中获取对应的 TypeVarId
                        if let Some(&type_var_id) = type_var_map_reverse.get(type_var_def) {
                            pin = pin.with_type_var_id(Some(type_var_id));
                        }
                    }
                }

                pin
            })
            .collect();

        // ---------- 收集 pin_ids ----------
        let pin_ids = pin_instances.iter().map(|p| p.id).collect();

        Ok(NodeCreationResult {
            node: Self {
                id: node_id,
                definition,
                type_var_map,
                position: NodePosition { x: 0.0, y: 0.0 },
                pin_ids,
            },
            pins: pin_instances,
        })
    }

    /// 设置位置
    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.position = NodePosition { x, y };
        self
    }
}
