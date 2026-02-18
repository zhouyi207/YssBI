use crate::graph::pin::PinDataTypeDefinition;
use crate::graph::value::DataType;
use crate::graph::PinInstance;
use crate::graph::{DataValue, NodeId, PinDirection, PinId, PinKind};
use serde::{Deserialize, Serialize};

/// 默认 Pin 类型颜色（与前端 ThemeSettings 一致）
const DEFAULT_COLORS: &[(&str, &str)] = &[
    ("exec", "#ffffff"),
    ("int", "#35b2b2"),
    ("float", "#9ecd4d"),
    ("bool", "#e06c75"),
    ("string", "#e5c07b"),
    ("date", "#c678dd"),
    ("datetime", "#c678dd"),
    ("dataframe", "#61afef"),
    ("dataseries", "#56b6c2"),
    ("object", "#abb2bf"),
    ("array", "#d19a66"),
];

/// 将 DataType 映射为前端期望的基础 pin type 字符串（用于颜色）
/// 容器类型（Array, DataSeries）会递归到内部类型
pub fn data_type_to_pin_type(dt: &DataType) -> &'static str {
    match dt {
        DataType::Boolean => "bool",
        DataType::Int32 | DataType::Int64 => "int",
        DataType::Float32 | DataType::Float64 => "float",
        DataType::String => "string",
        DataType::Array(inner) => data_type_to_pin_type(inner),
        DataType::Object => "object",
        DataType::Any => "any",
        DataType::DataFrame => "dataframe",
        DataType::DataSeries(inner) => data_type_to_pin_type(inner),
    }
}

/// 返回容器类型字符串（用于前端 pin 形状）
pub fn data_type_to_container(dt: &DataType) -> Option<&'static str> {
    match dt {
        DataType::Array(_) => Some("array"),
        DataType::DataSeries(_) => Some("dataseries"),
        _ => None,
    }
}

/// 根据 pin type 获取颜色
fn pin_type_to_color(pin_type: &str) -> Option<&'static str> {
    DEFAULT_COLORS
        .iter()
        .find(|(t, _)| *t == pin_type)
        .map(|(_, c)| *c)
}

/// 从 PinDataTypeDefinition 提取 DataType（仅 Concrete 类型）
fn definition_to_data_type(def: &PinDataTypeDefinition) -> Option<DataType> {
    match def {
        PinDataTypeDefinition::Concrete(dt) => Some(dt.clone()),
        _ => None,
    }
}

/// Pin UI 配置
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub default_value: Option<DataValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_value: Option<DataValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<PinUIDTO>,
}

impl PinInstanceDTO {
    /// 从 PinInstance 构建 DTO，支持传入已解析的类型和连接关系
    /// - resolved_type: 类型推断后的 DataType，优先于 definition 中的类型
    /// - links: 该 Pin 连接到的目标 Pin ID 列表
    pub fn from_pin_with_context(
        pin: &PinInstance,
        resolved_type: Option<&DataType>,
        links: Vec<PinId>,
    ) -> Self {
        let dt = match pin.definition.kind {
            PinKind::Exec => None,
            PinKind::Data => resolved_type
                .cloned()
                .or_else(|| pin.definition.data_type.as_ref().and_then(definition_to_data_type)),
        };

        let pin_type = match pin.definition.kind {
            PinKind::Exec => "exec".to_string(),
            PinKind::Data => match &dt {
                Some(d) => data_type_to_pin_type(d).to_string(),
                None => "object".to_string(),
            },
        };

        let container_type = dt.as_ref().and_then(|d| data_type_to_container(d).map(|s| s.to_string()));

        Self {
            id: pin.id,
            node_id: pin.node_id,
            name: pin.definition.name.clone(),
            pin_type,
            direction: pin.definition.direction,
            links,
            default_value: None,
            user_value: pin.user_value.clone(),
            container_type,
            ui: None,
        }
    }
}

impl From<&PinInstance> for PinInstanceDTO {
    fn from(value: &PinInstance) -> Self {
        Self::from_pin_with_context(value, None, Vec::new())
    }
}
