use crate::graph::PinInstance;
use crate::graph::pin::{PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::value::DataType;
use crate::graph::{DataValue, NodeId, PinDirection, PinId, PinKind};
use serde::{Deserialize, Serialize};

/// 将 DataType 映射为前端 pin type 字符串（运行时数值仅 Int64/Float64）
/// 容器类型（Array, DataSeries）会递归到内部类型
pub fn data_type_to_pin_type(dt: &DataType) -> &'static str {
    match dt {
        DataType::Boolean => "bool",
        DataType::Int64 => "Int64",
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<DataValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_value: Option<DataValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_display: Option<String>,
    /// 结构化类型（前端兼容判断的单一来源，serde 形如 {kind,inner}）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_type: Option<DataType>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<PinUIDTO>,
}

impl PinInstanceDTO {
    /// 从 PinInstance 构建 DTO，支持传入已解析的类型。
    /// - resolved_type: 类型推断后的 DataType，优先于 definition 中的类型
    pub fn from_pin_with_context(pin: &PinInstance, resolved_type: Option<&DataType>) -> Self {
        let dt = match pin.definition.kind {
            PinKind::Exec => None,
            PinKind::Data => resolved_type.cloned().or_else(|| {
                pin.definition
                    .data_type
                    .as_ref()
                    .and_then(definition_to_data_type)
            }),
        };

        let pin_type = match pin.definition.kind {
            PinKind::Exec => "exec".to_string(),
            PinKind::Data => match &dt {
                Some(d) => data_type_to_pin_type(d).to_string(),
                None => "object".to_string(),
            },
        };

        let container_type = dt
            .as_ref()
            .and_then(|d| data_type_to_container(d).map(|s| s.to_string()));
        let type_display = dt.as_ref().map(|d| d.to_string());

        Self {
            id: pin.id,
            node_id: pin.node_id,
            name: pin.definition.name.clone(),
            pin_type,
            direction: pin.definition.direction,
            default_value: pin.definition.default_value.clone(),
            user_value: pin.user_value.clone(),
            container_type,
            type_display,
            data_type: dt.clone(),
            optional: pin.definition.optional,
            ui: None,
        }
    }
}

impl From<&PinInstance> for PinInstanceDTO {
    fn from(value: &PinInstance) -> Self {
        Self::from_pin_with_context(value, None)
    }
}

/// Pin 槽位的前端 DTO（camelCase 字段，仅用于发往前端）
///
/// 与持久化用的 [`PinSlot`] 分离：`PinSlot` 在项目文件中以 snake_case 序列化，
/// 不能直接改名（会破坏旧项目读取）。前端渲染需要 camelCase，因此用独立 DTO。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "slotKind", rename_all = "camelCase")]
pub enum PinSlotDTO {
    Fixed {
        pin: PinDefinition,
    },
    Repeatable {
        template: PinDefinition,
        #[serde(rename = "namePrefix")]
        name_prefix: String,
        #[serde(rename = "minCount")]
        min_count: usize,
        #[serde(rename = "maxCount")]
        max_count: Option<usize>,
    },
    DerivedFromInput {
        #[serde(rename = "sourceRole")]
        source_role: PinRole,
        direction: PinDirection,
        #[serde(rename = "baseType")]
        base_type: PinDataTypeDefinition,
    },
}

impl From<&PinSlot> for PinSlotDTO {
    fn from(slot: &PinSlot) -> Self {
        match slot {
            PinSlot::Fixed { pin } => PinSlotDTO::Fixed { pin: pin.clone() },
            PinSlot::Repeatable {
                template,
                name_prefix,
                min_count,
                max_count,
            } => PinSlotDTO::Repeatable {
                template: template.clone(),
                name_prefix: name_prefix.clone(),
                min_count: *min_count,
                max_count: *max_count,
            },
            PinSlot::DerivedFromInput {
                source_role,
                direction,
                base_type,
            } => PinSlotDTO::DerivedFromInput {
                source_role: source_role.clone(),
                direction: *direction,
                base_type: base_type.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::pin::DataRole;

    /// 前端依赖 camelCase 字段名（namePrefix/minCount/maxCount）判断 repeatable pin 是否可移除。
    /// 锁定 DTO 的序列化契约，同时确认持久化 PinSlot 仍保持 snake_case。
    #[test]
    fn pin_slot_dto_serializes_camel_case() {
        let slot = PinSlot::repeatable(
            PinDefinition::data_input("", DataRole::Inputs(0), PinDataTypeDefinition::Unknown),
            "X",
            1,
            None,
        );

        let dto_json = serde_json::to_value(PinSlotDTO::from(&slot)).unwrap();
        assert_eq!(dto_json["slotKind"], "repeatable");
        assert_eq!(dto_json["namePrefix"], "X");
        assert_eq!(dto_json["minCount"], 1);
        assert!(dto_json.get("maxCount").is_some());
        assert!(dto_json.get("name_prefix").is_none());

        // 持久化类型保持 snake_case，避免破坏旧项目读取
        let persist_json = serde_json::to_value(&slot).unwrap();
        assert_eq!(persist_json["name_prefix"], "X");
        assert_eq!(persist_json["min_count"], 1);
        assert!(persist_json.get("namePrefix").is_none());
    }

    /// 前端兼容判断的单一来源是结构化 `dataType`：锁定 DTO 序列化形如
    /// `{kind:"DataSeries", inner:{kind:"Float64"}}`，与 Rust `DataType` serde 同源。
    #[test]
    fn pin_instance_dto_carries_structured_data_type() {
        let def =
            PinDefinition::data_input("col", DataRole::Inputs(0), PinDataTypeDefinition::Unknown);
        let pin = PinInstance::from_definition(&def, NodeId::new(), 0);
        let dt = DataType::DataSeries(Box::new(DataType::Float64));

        let dto = PinInstanceDTO::from_pin_with_context(&pin, Some(&dt));
        let json = serde_json::to_value(&dto).unwrap();

        assert_eq!(json["dataType"]["kind"], "DataSeries");
        assert_eq!(json["dataType"]["inner"]["kind"], "Float64");
        // typeDisplay 仍随结构化字段同源下发，作展示用
        assert_eq!(json["typeDisplay"], "DataSeries<Float64>");
        assert!(json.get("links").is_none());
    }
}
