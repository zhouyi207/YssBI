use crate::data_contract::{DataType, DataValue};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::{Add, Div, Mul, Sub};

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

fn default_array_value() -> DataValue {
    DataValue::Array(vec![
        DataValue::Int64(1),
        DataValue::Int64(2),
        DataValue::Int64(3),
    ])
}

fn default_object_value() -> DataValue {
    let mut map = HashMap::new();
    map.insert("key_0".to_owned(), DataValue::Int64(1));
    map.insert("key_1".to_owned(), DataValue::Int64(2));
    DataValue::Object(map)
}

impl DataType {
    pub fn default_value(&self) -> DataValue {
        match self {
            DataType::Boolean => DataValue::Boolean(false),
            DataType::Int64 => DataValue::Int64(0),
            DataType::Float64 => DataValue::Float64(0.0),
            DataType::String
            | DataType::Date
            | DataType::Datetime
            | DataType::Time
            | DataType::Categorical => DataValue::String(String::new()),
            DataType::Array(_) => default_array_value(),
            DataType::Object => default_object_value(),
            DataType::OneOf(types) => types
                .first()
                .map_or(DataValue::Null, DataType::default_value),
            DataType::Any | DataType::DataFrame | DataType::DataSeries(_) | DataType::Struct(_) => {
                DataValue::Null
            }
        }
    }

    pub fn is_primitive(&self) -> bool {
        match self {
            DataType::Boolean
            | DataType::Int64
            | DataType::Float64
            | DataType::String
            | DataType::Date
            | DataType::Datetime
            | DataType::Time
            | DataType::Categorical => true,
            DataType::OneOf(types) => !types.is_empty() && types.iter().all(DataType::is_primitive),
            DataType::Array(_)
            | DataType::Object
            | DataType::DataFrame
            | DataType::DataSeries(_)
            | DataType::Struct(_)
            | DataType::Any => false,
        }
    }

    pub fn is_numeric(&self) -> bool {
        match self {
            DataType::Int64 | DataType::Float64 => true,
            DataType::OneOf(types) => !types.is_empty() && types.iter().all(DataType::is_numeric),
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

    pub fn is_comparable(&self) -> bool {
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
                !types.is_empty() && types.iter().all(DataType::is_comparable)
            }
            DataType::Array(_)
            | DataType::Object
            | DataType::DataFrame
            | DataType::DataSeries(_)
            | DataType::Struct(_)
            | DataType::Any => false,
        }
    }

    pub fn is_iterable(&self) -> bool {
        match self {
            DataType::Array(_) | DataType::String | DataType::DataSeries(_) => true,
            DataType::OneOf(types) => !types.is_empty() && types.iter().all(DataType::is_iterable),
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

    pub fn can_convert(from: &DataType, to: &DataType) -> bool {
        if from == to {
            return true;
        }
        match (from, to) {
            (_, DataType::Any | DataType::String) => true,
            (_, DataType::OneOf(targets)) => targets
                .iter()
                .any(|target| DataType::can_convert(from, target)),
            (DataType::OneOf(sources), _) => sources
                .iter()
                .any(|source| DataType::can_convert(source, to)),
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

    pub fn can_accept(&self, source: &DataType) -> bool {
        TypeSystemSnapshot::empty().can_accept(self, source)
    }
}

impl DataValue {
    pub fn value_type(&self) -> Option<DataType> {
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

    pub fn as_bool(&self) -> Option<bool> {
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

    pub fn as_i64(&self) -> Option<i64> {
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

    pub fn as_f64(&self) -> Option<f64> {
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

    pub fn coerce_to(&self, target: &DataType) -> DataValue {
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

    pub fn add(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() + other.clone()
    }

    pub fn sub(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() - other.clone()
    }

    pub fn mul(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() * other.clone()
    }

    pub fn div(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() / other.clone()
    }
}

impl Add for DataValue {
    type Output = Result<DataValue, String>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
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
}

impl Sub for DataValue {
    type Output = Result<DataValue, String>;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
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
}

impl Mul for DataValue {
    type Output = Result<DataValue, String>;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
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
}

impl Div for DataValue {
    type Output = Result<DataValue, String>;

    fn div(self, rhs: Self) -> Self::Output {
        let is_zero = match &rhs {
            DataValue::Int64(value) => *value == 0,
            DataValue::Float64(value) => *value == 0.0,
            _ => false,
        };
        if is_zero {
            return Err("Division by zero".to_owned());
        }

        match (self, rhs) {
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
}

#[cfg(test)]
mod tests {
    use super::{StructTypeMeta, TypeSystemSnapshot};
    use crate::data_contract::DataType;
    use std::collections::BTreeMap;

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
