use crate::graph::pin::PinTypeCapability;
use crate::graph::{NodeDefinition, NodeInstanceParams, NodeMetaData, NodePosition};
use crate::graph::{NodeId, NodeInstance};
use crate::schema::pin::PinSlotDTO;
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
        Self {
            id: value.id,
            node_type: value.definition.node_type.clone(),
            category: value.definition.category.clone(),
            title: value.definition.name.clone(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            ui_style: value.definition.metadata.ui_style.clone(),
            description: value.definition.metadata.description.clone(),
            position: value.position.clone(),
            instance_params: value.instance_params.clone(),
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
    pub pin_slots: Vec<PinSlotDTO>,
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
            pin_slots: value.pin_slots.iter().map(PinSlotDTO::from).collect(),
            type_capabilities: value.type_capabilities(),
        }
    }
}
