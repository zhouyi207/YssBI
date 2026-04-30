use crate::graph::pin::{PinSlot, PinTypeCapability};
use crate::graph::{NodeDefinition, NodeInstanceParams, NodeMetaData, NodePosition};
use crate::graph::{NodeId, NodeInstance};
use serde::{Deserialize, Serialize};

/// Node instance DTO - 对应前端 Node 类型
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInstanceDTO {
    pub id: NodeId,
    pub node_type: String,
    pub category: Vec<String>,
    pub title: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub ui_style: String,
    pub description: Option<String>,
    pub position: NodePosition,
    #[serde(flatten)]
    pub instance_params: NodeInstanceParams,
}

impl From<&NodeInstance> for NodeInstanceDTO {
    fn from(value: &NodeInstance) -> Self {
        let p = &value.instance_params;
        let title = match value.definition.node_type.as_str() {
            "Variables:Get Variable" | "Variables:Set Variable" => {
                let prefix = if value.definition.node_type == "Variables:Get Variable" {
                    "Get"
                } else {
                    "Set"
                };
                p.variable_name()
                    .map(|n| format!("{} {}", prefix, n))
                    .unwrap_or_else(|| value.definition.name.clone())
            }
            "Data:Get DataFrame" => p
                .dataframe_name()
                .map(|n| format!("Get {}", n))
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

/// Node definition DTO - 用于节点注册（含完整 pin 槽位信息）
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NodeDefinitionDTO {
    pub name: String,
    pub category: Vec<String>,
    pub node_type: String,
    pub node_metadata: NodeMetaData,
    /// 声明式 pin 槽位（前端可用于渲染和兼容性过滤）
    pub pin_slots: Vec<PinSlot>,
    /// 预计算的类型能力（前端拖 pin 过滤用）
    pub type_capabilities: Vec<PinTypeCapability>,
}

impl From<&NodeDefinition> for NodeDefinitionDTO {
    fn from(value: &NodeDefinition) -> Self {
        Self {
            name: value.name.clone(),
            category: value.category.clone(),
            node_type: value.node_type.clone(),
            node_metadata: value.metadata.clone(),
            pin_slots: value.pin_slots.clone(),
            type_capabilities: value.type_capabilities(),
        }
    }
}
