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

/// 节点实例参数（用于 variable、function、macro、dataframe 等需要运行时绑定的节点）
///
/// 新增参数只需在此处添加字段，`NodeInstanceDTO` 通过 `#[serde(flatten)]` 自动展开。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInstanceParams {
    /// 变量节点：绑定的变量 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_id: Option<String>,
    /// 变量节点：变量名称（用于 UI 显示）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_name: Option<String>,
    /// 变量节点：变量类型（用于 UI 显示）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_type: Option<String>,
    /// 函数/宏调用节点：子图 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_graph_id: Option<String>,
    /// DataFrame 节点：数据帧 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataframe_id: Option<String>,
    /// Get Column 节点：列名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_name: Option<String>,
    /// Get Column 节点：列类型
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_type: Option<String>,
}

impl NodeInstanceParams {
    /// 所有字段都是 None 时返回 true
    pub fn is_empty(&self) -> bool {
        self.variable_id.is_none()
            && self.variable_name.is_none()
            && self.variable_type.is_none()
            && self.sub_graph_id.is_none()
            && self.dataframe_id.is_none()
            && self.column_name.is_none()
            && self.column_type.is_none()
    }
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

    /// 实例参数（variable_id, sub_graph_id 等）
    #[serde(default)]
    pub instance_params: NodeInstanceParams,

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
                instance_params: NodeInstanceParams::default(),
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

    /// 设置实例参数
    pub fn with_instance_params(mut self, params: NodeInstanceParams) -> Self {
        self.instance_params = params;
        self
    }
}
