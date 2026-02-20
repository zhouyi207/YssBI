use crate::graph::{NodeDefinition, NodeMetaData, NodePosition, NodeInstanceParams};
use crate::graph::{NodeId, NodeInstance};
use serde::{Deserialize, Serialize};

/// Node instance DTO - 对应前端 Node 类型
///
/// 实例参数（variable_id, sub_graph_id 等）通过 `#[serde(flatten)]` 自动展开到 JSON 顶层，
/// 新增参数只需修改 `NodeInstanceParams`，此处无需改动。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInstanceDTO {
    pub id: NodeId,
    pub node_type: String,
    pub category: Vec<String>,
    pub title: String,
    pub inputs: Vec<String>,  // Pin IDs
    pub outputs: Vec<String>, // Pin IDs
    pub ui_style: String,
    pub description: Option<String>,
    pub position: NodePosition,
    /// 实例参数（flatten 到 JSON 顶层，保持前端兼容）
    #[serde(flatten)]
    pub instance_params: NodeInstanceParams,
}

impl From<&NodeInstance> for NodeInstanceDTO {
    fn from(value: &NodeInstance) -> Self {
        let p = &value.instance_params;
        let title = match value.definition.node_type.as_str() {
            "get_variable" | "set_variable" => {
                let prefix = if value.definition.node_type == "get_variable" { "Get" } else { "Set" };
                p.variable_name()
                    .map(|n| format!("{} {}", prefix, n))
                    .unwrap_or_else(|| value.definition.name.clone())
            }
            "get_dataframe" => p
                .dataframe_id()
                .map(|_| value.definition.name.clone())
                .unwrap_or_else(|| value.definition.name.clone()),
            _ => value.definition.name.clone(),
        };
        Self {
            id: value.id,
            node_type: value.definition.node_type.clone(),
            category: value.definition.category.clone(),
            title,
            inputs: Vec::new(),
            outputs: Vec::new(),
            ui_style: value.definition.metadata.ui_style.clone(),
            description: value.definition.metadata.description.clone(),
            position: value.position.clone(),
            instance_params: p.clone(),
        }
    }
}

/// Node definition DTO - 用于节点注册
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NodeDefinitionDTO {
    pub name: String,
    pub category: Vec<String>,
    pub node_type: String,
    pub node_metadata: NodeMetaData,
}

impl From<&NodeDefinition> for NodeDefinitionDTO {
    fn from(value: &NodeDefinition) -> Self {
        Self {
            name: value.name.clone(),
            category: value.category.clone(),
            node_type: value.node_type.clone(),
            node_metadata: value.metadata.clone(),
        }
    }
}
