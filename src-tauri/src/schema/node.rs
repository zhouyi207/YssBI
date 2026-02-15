use crate::graph::{NodeDefinition, NodeMetaData, NodePosition};
use crate::graph::{NodeId, NodeInstance};
use serde::{Deserialize, Serialize};

/// Node instance DTO - 对应前端 Node 类型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInstanceDTO {
    pub id: NodeId,
    #[serde(rename = "node_type")]
    pub node_type: String,
    pub category: Vec<String>,
    pub title: String,
    pub inputs: Vec<String>,  // Pin IDs
    pub outputs: Vec<String>, // Pin IDs
    pub ui_style: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub position: NodePosition,  // 添加位置信息
}

impl From<&NodeInstance> for NodeInstanceDTO {
    fn from(value: &NodeInstance) -> Self {
        Self {
            id: value.id,
            node_type: value.definition.name.clone(),
            category: value.definition.category.clone(),
            title: value.definition.name.clone(),
            inputs: Vec::new(),  // 需要从 GraphDataState 中获取
            outputs: Vec::new(), // 需要从 GraphDataState 中获取
            ui_style: value.definition.metadata.ui_style.clone(),
            description: value.definition.metadata.description.clone(),
            position: value.position.clone(),
        }
    }
}

/// Node definition DTO - 用于节点注册
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeDefinitionDTO {
    pub name: String,
    pub category: Vec<String>,
    pub node_metadata: NodeMetaData,
}

impl From<&NodeDefinition> for NodeDefinitionDTO {
    fn from(value: &NodeDefinition) -> Self {
        Self {
            name: value.name.clone(),
            category: value.category.clone(),
            node_metadata: value.metadata.clone(),
        }
    }
}
