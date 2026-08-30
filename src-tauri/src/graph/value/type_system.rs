use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use yss_data_contract::{DataType, DataValue};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructTypeMeta {
    pub key: String,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeSystemSnapshot {
    pub struct_types: BTreeMap<String, StructTypeMeta>,
}

impl TypeSystemSnapshot {
    pub fn empty() -> Self {
        Self {
            struct_types: BTreeMap::new(),
        }
    }

    pub fn can_accept(&self, target: &DataType, source: &DataType) -> bool {
        if target == source {
            return true;
        }
        if matches!(target, DataType::Any) || matches!(source, DataType::Any) {
            return true;
        }
        match (source, target) {
            (_, DataType::OneOf(targets)) => {
                targets.iter().any(|item| self.can_accept(item, source))
            }
            (DataType::OneOf(sources), _) => {
                sources.iter().any(|item| self.can_accept(target, item))
            }
            (DataType::Array(source_inner), DataType::Array(target_inner)) => {
                self.can_accept(target_inner, source_inner)
            }
            (DataType::DataSeries(source_inner), DataType::DataSeries(target_inner)) => {
                self.can_accept(target_inner, source_inner)
            }
            (DataType::Struct(source_key), DataType::Struct(target_key)) => {
                target_key == source_key || struct_extends(source_key, target_key, self)
            }
            _ => false,
        }
    }
}

fn struct_extends(source_key: &str, target_key: &str, snapshot: &TypeSystemSnapshot) -> bool {
    let mut visited = HashSet::new();
    let mut stack = vec![source_key];

    while let Some(key) = stack.pop() {
        if !visited.insert(key.to_owned()) {
            continue;
        }
        let Some(meta) = snapshot.struct_types.get(key) else {
            continue;
        };
        for parent in &meta.parents {
            if parent == target_key {
                return true;
            }
            stack.push(parent);
        }
    }

    false
}

pub trait DataTypeBehavior {
    fn is_primitive(&self) -> bool;
    fn is_numeric(&self) -> bool;
    fn is_comparable(&self) -> bool;
    fn is_iterable(&self) -> bool;
    fn can_convert(from: &DataType, to: &DataType) -> bool;
    fn can_accept(&self, source: &DataType) -> bool;
}

impl DataTypeBehavior for DataType {
    fn is_primitive(&self) -> bool {
        match self {
            DataType::Boolean
            | DataType::Int64
            | DataType::Float64
            | DataType::String
            | DataType::Date
            | DataType::Datetime
            | DataType::Time
            | DataType::Categorical => true,
            DataType::OneOf(types) => {
                !types.is_empty() && types.iter().all(DataTypeBehavior::is_primitive)
            }
            DataType::Array(_)
            | DataType::Object
            | DataType::DataFrame
            | DataType::DataSeries(_)
            | DataType::Struct(_)
            | DataType::Any => false,
        }
    }

    fn is_numeric(&self) -> bool {
        match self {
            DataType::Int64 | DataType::Float64 => true,
            DataType::OneOf(types) => {
                !types.is_empty() && types.iter().all(DataTypeBehavior::is_numeric)
            }
            DataType::Boolean
            | DataType::String
            | DataType::Date
            | DataType::Datetime
            | DataType::Time
            | DataType::Categorical
            | DataType::Array(_)
            | DataType::Object
            | DataType::DataFrame
            | DataType::DataSeries(_)
            | DataType::Struct(_)
            | DataType::Any => false,
        }
    }

    fn is_comparable(&self) -> bool {
        match self {
            DataType::Boolean
            | DataType::Int64
            | DataType::Float64
            | DataType::String
            | DataType::Date
            | DataType::Datetime
            | DataType::Time
            | DataType::Categorical => true,
            DataType::OneOf(types) => {
                !types.is_empty() && types.iter().all(DataTypeBehavior::is_comparable)
            }
            DataType::Array(_)
            | DataType::Object
            | DataType::DataFrame
            | DataType::DataSeries(_)
            | DataType::Struct(_)
            | DataType::Any => false,
        }
    }

    fn is_iterable(&self) -> bool {
        match self {
            DataType::Array(_) | DataType::String | DataType::DataSeries(_) => true,
            DataType::OneOf(types) => {
                !types.is_empty() && types.iter().all(DataTypeBehavior::is_iterable)
            }
            DataType::Boolean
            | DataType::Int64
            | DataType::Float64
            | DataType::Date
            | DataType::Datetime
            | DataType::Time
            | DataType::Categorical
            | DataType::Object
            | DataType::DataFrame
            | DataType::Struct(_)
            | DataType::Any => false,
        }
    }

    fn can_convert(from: &DataType, to: &DataType) -> bool {
        if from == to {
            return true;
        }
        match (from, to) {
            (_, DataType::Any | DataType::String) => true,
            (_, DataType::OneOf(targets)) => {
                targets.iter().any(|target| Self::can_convert(from, target))
            }
            (DataType::OneOf(sources), _) => {
                sources.iter().any(|source| Self::can_convert(source, to))
            }
            (
                _,
                DataType::Boolean
                | DataType::Int64
                | DataType::Float64
                | DataType::Date
                | DataType::Datetime
                | DataType::Time
                | DataType::Categorical,
            ) => from.is_primitive(),
            (
                _,
                DataType::Array(_)
                | DataType::Object
                | DataType::DataFrame
                | DataType::DataSeries(_)
                | DataType::Struct(_),
            ) => false,
        }
    }

    fn can_accept(&self, source: &DataType) -> bool {
        TypeSystemSnapshot::empty().can_accept(self, source)
    }
}

pub trait DataValueBehavior {
    fn value_type(&self) -> Option<DataType>;
    fn as_bool(&self) -> Option<bool>;
    fn as_i64(&self) -> Option<i64>;
    fn as_f64(&self) -> Option<f64>;
    fn coerce_to(&self, target: &DataType) -> DataValue;
    fn add(&self, other: &DataValue) -> Result<DataValue, String>;
    fn sub(&self, other: &DataValue) -> Result<DataValue, String>;
    fn mul(&self, other: &DataValue) -> Result<DataValue, String>;
    fn div(&self, other: &DataValue) -> Result<DataValue, String>;
}

impl DataValueBehavior for DataValue {
    fn value_type(&self) -> Option<DataType> {
        match self {
            DataValue::Boolean(_) => Some(DataType::Boolean),
            DataValue::Int64(_) => Some(DataType::Int64),
            DataValue::Float64(_) => Some(DataType::Float64),
            DataValue::String(_) => Some(DataType::String),
            DataValue::Array(values) => {
                let inner = values
                    .iter()
                    .find_map(DataValue::value_type)
                    .unwrap_or(DataType::Any);
                Some(DataType::Array(Box::new(inner)))
            }
            DataValue::Object(_) => Some(DataType::Object),
            DataValue::DataFrame(_) => Some(DataType::DataFrame),
            DataValue::DataSeries(series) => Some(DataType::DataSeries(Box::new(
                series.element_type.clone().unwrap_or(DataType::Any),
            ))),
            DataValue::Struct { type_key, .. } => Some(DataType::Struct(type_key.clone())),
            DataValue::Null => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            DataValue::Boolean(value) => Some(*value),
            DataValue::Int64(value) => Some(*value != 0),
            DataValue::Float64(value) => Some(*value != 0.0),
            DataValue::String(value) => Some(!value.is_empty()),
            DataValue::Null => Some(false),
            DataValue::Array(_)
            | DataValue::Object(_)
            | DataValue::DataFrame(_)
            | DataValue::DataSeries(_)
            | DataValue::Struct { .. } => None,
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self {
            DataValue::Int64(value) => Some(*value),
            DataValue::Float64(value) => Some(*value as i64),
            DataValue::Boolean(value) => Some(if *value { 1 } else { 0 }),
            DataValue::String(_)
            | DataValue::Array(_)
            | DataValue::Object(_)
            | DataValue::DataFrame(_)
            | DataValue::DataSeries(_)
            | DataValue::Struct { .. }
            | DataValue::Null => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self {
            DataValue::Float64(value) => Some(*value),
            DataValue::Int64(value) => Some(*value as f64),
            DataValue::Boolean(value) => Some(if *value { 1.0 } else { 0.0 }),
            DataValue::String(_)
            | DataValue::Array(_)
            | DataValue::Object(_)
            | DataValue::DataFrame(_)
            | DataValue::DataSeries(_)
            | DataValue::Struct { .. }
            | DataValue::Null => None,
        }
    }

    fn coerce_to(&self, target: &DataType) -> DataValue {
        if self.value_type().is_some_and(|source| source == *target) {
            return self.clone();
        }

        match target {
            DataType::Boolean => self
                .as_bool()
                .map(DataValue::Boolean)
                .unwrap_or_else(|| self.clone()),
            DataType::Int64 => self
                .as_i64()
                .map(DataValue::Int64)
                .unwrap_or_else(|| self.clone()),
            DataType::Float64 => self
                .as_f64()
                .map(DataValue::Float64)
                .unwrap_or_else(|| self.clone()),
            DataType::Date
            | DataType::Datetime
            | DataType::Time
            | DataType::String
            | DataType::Categorical => match self {
                DataValue::String(value) => DataValue::String(value.clone()),
                DataValue::Boolean(value) => DataValue::String(value.to_string()),
                DataValue::Int64(value) => DataValue::String(value.to_string()),
                DataValue::Float64(value) => DataValue::String(value.to_string()),
                DataValue::DataFrame(id) => DataValue::String(format!("DataFrame({id})")),
                DataValue::DataSeries(series) => {
                    DataValue::String(format!("DataSeries({})", series.id))
                }
                DataValue::Struct {
                    type_key,
                    handle_id,
                } => DataValue::String(format!("Struct<{type_key}>({handle_id})")),
                DataValue::Null => DataValue::String("null".to_owned()),
                DataValue::Array(_) | DataValue::Object(_) => self.clone(),
            },
            DataType::Array(target_inner) => match self {
                DataValue::Array(values) => DataValue::Array(
                    values
                        .iter()
                        .map(|value| value.coerce_to(target_inner))
                        .collect(),
                ),
                _ => self.clone(),
            },
            DataType::Any
            | DataType::Object
            | DataType::DataFrame
            | DataType::DataSeries(_)
            | DataType::Struct(_)
            | DataType::OneOf(_) => self.clone(),
        }
    }

    fn add(&self, other: &DataValue) -> Result<DataValue, String> {
        add_values(self.clone(), other.clone())
    }

    fn sub(&self, other: &DataValue) -> Result<DataValue, String> {
        subtract_values(self.clone(), other.clone())
    }

    fn mul(&self, other: &DataValue) -> Result<DataValue, String> {
        multiply_values(self.clone(), other.clone())
    }

    fn div(&self, other: &DataValue) -> Result<DataValue, String> {
        divide_values(self.clone(), other.clone())
    }
}

fn add_values(left: DataValue, right: DataValue) -> Result<DataValue, String> {
    match (left, right) {
        (DataValue::Int64(left), DataValue::Int64(right)) => Ok(DataValue::Int64(left + right)),
        (DataValue::Float64(left), DataValue::Float64(right)) => {
            Ok(DataValue::Float64(left + right))
        }
        (DataValue::String(left), DataValue::String(right)) => {
            Ok(DataValue::String(format!("{left}{right}")))
        }
        (left, right) => Err(format!(
            "Cannot add {:?} and {:?}: incompatible types",
            left.value_type(),
            right.value_type()
        )),
    }
}

fn subtract_values(left: DataValue, right: DataValue) -> Result<DataValue, String> {
    match (left, right) {
        (DataValue::Int64(left), DataValue::Int64(right)) => Ok(DataValue::Int64(left - right)),
        (DataValue::Float64(left), DataValue::Float64(right)) => {
            Ok(DataValue::Float64(left - right))
        }
        (left, right) => Err(format!(
            "Cannot subtract {:?} from {:?}: incompatible types",
            right.value_type(),
            left.value_type()
        )),
    }
}

fn multiply_values(left: DataValue, right: DataValue) -> Result<DataValue, String> {
    match (left, right) {
        (DataValue::Int64(left), DataValue::Int64(right)) => Ok(DataValue::Int64(left * right)),
        (DataValue::Float64(left), DataValue::Float64(right)) => {
            Ok(DataValue::Float64(left * right))
        }
        (left, right) => Err(format!(
            "Cannot multiply {:?} and {:?}: incompatible types",
            left.value_type(),
            right.value_type()
        )),
    }
}

fn divide_values(left: DataValue, right: DataValue) -> Result<DataValue, String> {
    let is_zero = match &right {
        DataValue::Int64(value) => *value == 0,
        DataValue::Float64(value) => *value == 0.0,
        _ => false,
    };
    if is_zero {
        return Err("Division by zero".to_owned());
    }

    match (left, right) {
        (DataValue::Int64(left), DataValue::Int64(right)) => Ok(DataValue::Int64(left / right)),
        (DataValue::Float64(left), DataValue::Float64(right)) => {
            Ok(DataValue::Float64(left / right))
        }
        (left, right) => Err(format!(
            "Cannot divide {:?} by {:?}: incompatible types",
            left.value_type(),
            right.value_type()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{DataTypeBehavior, StructTypeMeta, TypeSystemSnapshot};
    use std::collections::BTreeMap;
    use yss_data_contract::DataType;

    fn model_type_system() -> TypeSystemSnapshot {
        let mut struct_types = BTreeMap::new();
        struct_types.insert(
            "Model".to_owned(),
            StructTypeMeta {
                key: "Model".to_owned(),
                parents: vec![],
                category: Some("model".to_owned()),
                display_name: Some("Model".to_owned()),
            },
        );
        struct_types.insert(
            "OLSModel".to_owned(),
            StructTypeMeta {
                key: "OLSModel".to_owned(),
                parents: vec!["Model".to_owned()],
                category: Some("model".to_owned()),
                display_name: Some("OLS Model".to_owned()),
            },
        );
        TypeSystemSnapshot { struct_types }
    }

    #[test]
    fn number_display_string_parses_as_the_canonical_numeric_union() {
        let parsed = "Number"
            .parse::<DataType>()
            .expect("Number must parse as the canonical numeric union");

        assert_eq!(parsed, DataType::number());
        assert_eq!(
            parsed
                .to_string()
                .parse::<DataType>()
                .expect("the displayed numeric union must parse"),
            parsed
        );
    }

    #[test]
    fn data_type_struct_acceptance_is_exact_without_type_system() {
        let target = DataType::Struct("Model".to_owned());
        let source = DataType::Struct("OLSModel".to_owned());

        assert!(!target.can_accept(&source));
    }

    #[test]
    fn type_system_accepts_concrete_ols_model_for_model_family() {
        let type_system = model_type_system();
        let target = DataType::Struct("Model".to_owned());
        let source = DataType::Struct("OLSModel".to_owned());

        assert!(type_system.can_accept(&target, &source));
    }

    #[test]
    fn type_system_rejects_unrelated_structs_for_model_family() {
        let type_system = model_type_system();
        let target = DataType::Struct("Model".to_owned());
        let source = DataType::Struct("OLSResult".to_owned());

        assert!(!type_system.can_accept(&target, &source));
    }
}
