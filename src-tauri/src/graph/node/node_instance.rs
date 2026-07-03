//! Node 实例（运行时）

use super::{NodeDefinition, NodePosition};
use crate::graph::TypeVarDefinition;
use crate::graph::node::NodeId;
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

/// 节点实例参数（Tagged Enum）
///
/// 每种参数化节点类型对应一个变体，编译期保证类型安全。
/// 通过 `#[serde(tag = "paramsKind")]` + `#[serde(flatten)]` 展开到 DTO JSON 顶层。
///
/// 前端 DTO 格式示例：
/// - `{ paramsKind: "none" }`
/// - `{ paramsKind: "variable", variableId: "...", variableName: "...", variableType: "..." }`
/// - `{ paramsKind: "subGraph", subGraphId: "..." }`
/// - `{ paramsKind: "dataFrame", dataframeId: "..." }`
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "paramsKind")]
pub enum NodeInstanceParams {
    /// 无特殊参数的节点
    #[serde(rename = "none")]
    None,

    /// 变量节点（get_variable / set_variable）
    #[serde(rename = "variable")]
    Variable {
        #[serde(rename = "variableId")]
        variable_id: String,
        #[serde(rename = "variableName", skip_serializing_if = "Option::is_none")]
        variable_name: Option<String>,
        #[serde(rename = "variableType", skip_serializing_if = "Option::is_none")]
        variable_type: Option<String>,
    },

    /// 函数调用节点（call_function）
    #[serde(rename = "subGraph")]
    SubGraph {
        #[serde(rename = "subGraphId")]
        sub_graph_id: String,
    },

    /// DataFrame 节点（get_dataframe）
    #[serde(rename = "dataFrame")]
    DataFrame {
        #[serde(rename = "dataframeId")]
        dataframe_id: String,
    },
}

impl Default for NodeInstanceParams {
    fn default() -> Self {
        NodeInstanceParams::None
    }
}

impl NodeInstanceParams {
    pub fn is_empty(&self) -> bool {
        matches!(self, NodeInstanceParams::None)
    }

    /// 便捷方法：获取 variable_id（仅 Variable 变体）
    pub fn variable_id(&self) -> Option<&str> {
        match self {
            NodeInstanceParams::Variable { variable_id, .. } => Some(variable_id),
            _ => Option::None,
        }
    }

    /// 便捷方法：获取 sub_graph_id（仅 SubGraph 变体）
    pub fn sub_graph_id(&self) -> Option<&str> {
        match self {
            NodeInstanceParams::SubGraph { sub_graph_id } => Some(sub_graph_id),
            _ => Option::None,
        }
    }

    /// 便捷方法：获取 dataframe_id（仅 DataFrame 变体）
    pub fn dataframe_id(&self) -> Option<&str> {
        match self {
            NodeInstanceParams::DataFrame { dataframe_id } => Some(dataframe_id),
            _ => Option::None,
        }
    }
}

/// Node 实例（运行时）
#[derive(Clone, Debug)]
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
        let pin_defs = definition.generate_initial_pins()?;

        let pin_instances: Vec<PinInstance> = pin_defs
            .iter()
            .enumerate()
            .map(|(index, pin_definition)| {
                let order = index as i32;

                let mut pin = PinInstance::from_definition(pin_definition, node_id, order);

                // 如果 pin 使用了类型变量，设置 type_var_id
                if let Some(type_var_key) = pin_definition.get_type_var_key() {
                    // 从 NodeDefinition.type_vars 中找到对应的 TypeVarDefinition
                    if let Some(type_var_def) =
                        definition.type_vars.iter().find(|tv| tv.id == type_var_key)
                    {
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

    /// Create an instance with predetermined node ID and pin IDs (for redo / restore).
    /// The caller must supply exactly the right number of pin IDs matching the definition.
    pub fn from_definition_with_ids(
        definition: Arc<NodeDefinition>,
        node_id: NodeId,
        pin_ids: &[PinId],
    ) -> Result<NodeCreationResult, String> {
        let mut type_var_map = HashMap::new();
        let mut type_var_map_reverse = HashMap::new();

        for type_var in definition.type_vars.iter().cloned() {
            let type_var_id = TypeVarId::new();
            type_var_map.insert(type_var_id, type_var.clone());
            type_var_map_reverse.insert(type_var, type_var_id);
        }

        let pin_defs = definition.generate_initial_pins()?;

        if pin_ids.len() != pin_defs.len() {
            return Err(format!(
                "Expected {} pin IDs, got {}",
                pin_defs.len(),
                pin_ids.len()
            ));
        }

        let pin_instances: Vec<PinInstance> = pin_defs
            .iter()
            .enumerate()
            .map(|(index, pin_definition)| {
                let order = index as i32;
                let mut pin = PinInstance::from_definition(pin_definition, node_id, order);
                pin.id = pin_ids[index];

                if let Some(type_var_key) = pin_definition.get_type_var_key() {
                    if let Some(type_var_def) =
                        definition.type_vars.iter().find(|tv| tv.id == type_var_key)
                    {
                        if let Some(&type_var_id) = type_var_map_reverse.get(type_var_def) {
                            pin = pin.with_type_var_id(Some(type_var_id));
                        }
                    }
                }

                pin
            })
            .collect();

        let collected_pin_ids = pin_instances.iter().map(|p| p.id).collect();

        Ok(NodeCreationResult {
            node: Self {
                id: node_id,
                definition,
                type_var_map,
                position: NodePosition { x: 0.0, y: 0.0 },
                instance_params: NodeInstanceParams::default(),
                pin_ids: collected_pin_ids,
            },
            pins: pin_instances,
        })
    }

    /// Create an instance with a predetermined node ID but auto-generated pin IDs.
    /// Used when restoring nodes with saved pin snapshots (dynamic / repeatable pins).
    /// may differ from the base definition.
    pub fn from_definition_with_node_id(
        definition: Arc<NodeDefinition>,
        node_id: NodeId,
    ) -> Result<NodeCreationResult, String> {
        let mut type_var_map = HashMap::new();
        let mut type_var_map_reverse = HashMap::new();

        for type_var in definition.type_vars.iter().cloned() {
            let type_var_id = TypeVarId::new();
            type_var_map.insert(type_var_id, type_var.clone());
            type_var_map_reverse.insert(type_var, type_var_id);
        }

        let pin_defs = definition.generate_initial_pins()?;

        let pin_instances: Vec<PinInstance> = pin_defs
            .iter()
            .enumerate()
            .map(|(index, pin_definition)| {
                let order = index as i32;
                let mut pin = PinInstance::from_definition(pin_definition, node_id, order);

                if let Some(type_var_key) = pin_definition.get_type_var_key() {
                    if let Some(type_var_def) =
                        definition.type_vars.iter().find(|tv| tv.id == type_var_key)
                    {
                        if let Some(&type_var_id) = type_var_map_reverse.get(type_var_def) {
                            pin = pin.with_type_var_id(Some(type_var_id));
                        }
                    }
                }

                pin
            })
            .collect();

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

// `NodeInstance` 不再单独序列化：磁盘格式由 `GraphInstance` 的自定义 serde 统一
// 负责（节点以 `nodeType` 落盘，完整定义在加载后由 registry 重挂）。

#[cfg(test)]
mod tests {
    use super::NodeInstanceParams;

    #[test]
    fn dataframe_params_serialize_only_stable_id() {
        let params = NodeInstanceParams::DataFrame {
            dataframe_id: "df-1".to_string(),
        };

        let value = serde_json::to_value(params).unwrap();

        assert_eq!(value["paramsKind"], "dataFrame");
        assert_eq!(value["dataframeId"], "df-1");
        assert!(value.get("dataframeName").is_none());
    }
}
