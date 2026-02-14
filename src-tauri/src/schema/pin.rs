use crate::graph::PinInstance;
use crate::graph::{NodeId, PinDirection, PinId, PinKind};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Pin UI 配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PinUIDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Pin instance DTO - 对应前端 Pin 类型
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinInstanceDTO {
    pub id: PinId,
    pub node_id: NodeId,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    pub direction: PinDirection,
    pub links: Vec<PinId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_value: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_array: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<PinUIDTO>,
}

impl From<&PinInstance> for PinInstanceDTO {
    fn from(value: &PinInstance) -> Self {
        // 将 PinKind 转换为前端的 type 字符串
        let pin_type = match value.definition.kind {
            PinKind::Exec => "exec".to_string(),
            PinKind::Data => {
                // 从 data_type 获取具体类型
                if let Some(ref data_type) = value.definition.data_type {
                    format!("{:?}", data_type).to_lowercase()
                } else {
                    "any".to_string()
                }
            }
        };

        Self {
            id: value.id,
            node_id: value.node_id,
            name: value.definition.name.clone(),
            pin_type,
            direction: value.definition.direction,
            links: Vec::new(), // 需要从 ConnectionManager 中获取
            default_value: None, // TODO: 从 PinDefinition 获取
            user_value: None,    // TODO: 从 PinRuntimeState 获取
            is_array: None,      // TODO: 从 PinDefinition 获取
            ui: None,
        }
    }
}
