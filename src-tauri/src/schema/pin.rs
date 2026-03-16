use crate::graph::pin::PinDataTypeDefinition;
use crate::graph::value::DataType;
use crate::graph::PinInstance;
use crate::graph::{DataValue, NodeId, PinDirection, PinId, PinKind};
use serde::{Deserialize, Serialize};

/// 将 DataType 映射为前端 pin type 字符串（保留精度：Int32/Int64/Float32/Float64）
/// 容器类型（Array, DataSeries）会递归到内部类型
pub fn data_type_to_pin_type(dt: &DataType) -> &'static str {
    match dt {
        DataType::Boolean => "bool",
        DataType::Int32 => "Int32",
        DataType::Int64 => "Int64",
        DataType::Float32 => "Float32",
        DataType::Float64 => "Float64",
        DataType::String => "string",
        DataType::Date => "date",
        DataType::Categorical => "categorical",
        DataType::Array(inner) => data_type_to_pin_type(inner),
        DataType::Object => "object",
        DataType::Any => "any",
        DataType::DataFrame => "dataframe",
        DataType::DataSeries(inner) => data_type_to_pin_type(inner),
        DataType::Struct(_) => "struct",
        DataType::OneOf(_) => "oneof",
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

fn is_false(v: &bool) -> bool {
    !v
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
    pub type_display: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
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
        let type_display = dt.as_ref().map(|d| d.to_string());

        Self {
            id: pin.id,
            node_id: pin.node_id,
            name: pin.definition.name.clone(),
            pin_type,
            direction: pin.definition.direction,
            links,
            default_value: pin.definition.default_value.clone(),
            user_value: pin.user_value.clone(),
            container_type,
            type_display,
            optional: pin.definition.optional,
            ui: None,
        }
    }
}

impl From<&PinInstance> for PinInstanceDTO {
    fn from(value: &PinInstance) -> Self {
        Self::from_pin_with_context(value, None, Vec::new())
    }
}
