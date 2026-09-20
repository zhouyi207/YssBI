use crate::{ConstantId, GraphConstant};
use serde::Deserializer as _;
use serde::de::{MapAccess, Visitor};
use serde_json::Value;
use std::fmt;
use yss_data_contract::{DataValue, ValueType};
use yss_data_contract::{TabularColumn, TabularContractError, TabularScalar, TabularSnapshot};

/// Returns an inert value suitable for a constant whose type has just changed.
///
/// Compound defaults are deliberately empty. Populating arrays or objects with
/// sample data would invent user data and can violate the declared element type.
pub fn default_value_for(data_type: &ValueType) -> DataValue {
    use yss_data_contract::SemanticType;
    match data_type {
        ValueType::Scalar(SemanticType::Binary) => DataValue::Bool(false),
        ValueType::Scalar(SemanticType::Numeric) => DataValue::Integer(0),
        ValueType::Scalar(_) => DataValue::String("".into()),
        ValueType::Array(_) => DataValue::List(Vec::new()),
        ValueType::Object => DataValue::Object(std::collections::BTreeMap::new()),
        ValueType::OneOf(types) => types.first().map_or(DataValue::Null, default_value_for),
        _ => DataValue::Null,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConstantValueError {
    #[error("constant value does not match its declared type")]
    ValueKindMismatch,
    #[error("tabular constant JSON is invalid")]
    InvalidJson,
    #[error("tabular constant JSON must be a column map")]
    ExpectedColumnMap,
    #[error("tabular constant column must be an array")]
    ColumnNotArray {
        column: yss_data_contract::TabularColumnName,
    },
    #[error("tabular constant cell must be a scalar")]
    UnsupportedCell {
        column: yss_data_contract::TabularColumnName,
        row: usize,
    },
    #[error("tabular constant contract is invalid")]
    Contract(TabularContractError),
}

impl GraphConstant {
    pub fn copy_with_id(&self, id: ConstantId) -> Self {
        Self { id, ..self.clone() }
    }
}

fn parse_literal(payload: &str) -> Result<TabularSnapshot, ConstantValueError> {
    let parsed: Value =
        serde_json::from_str(payload).map_err(|_| ConstantValueError::InvalidJson)?;
    let Value::Object(columns) = &parsed else {
        return Err(ConstantValueError::ExpectedColumnMap);
    };
    for (name, values) in columns {
        let column = yss_data_contract::TabularColumnName::try_from(name.as_str())
            .map_err(ConstantValueError::Contract)?;
        let Some(values) = values.as_array() else {
            return Err(ConstantValueError::ColumnNotArray { column });
        };
        if let Some(row) = values.iter().position(|value| {
            !matches!(
                value,
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
            )
        }) {
            return Err(ConstantValueError::UnsupportedCell { column, row });
        }
    }

    let columns =
        deserialize_literal_columns(payload).map_err(|_| ConstantValueError::InvalidJson)?;

    TabularSnapshot::try_from_columns(
        columns
            .into_iter()
            .map(|(name, values)| {
                yss_data_contract::TabularColumnName::try_from(name.as_str())
                    .map(|name| TabularColumn::new(name, values.into_boxed_slice()))
                    .map_err(ConstantValueError::Contract)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice(),
    )
    .map_err(ConstantValueError::Contract)
}

fn deserialize_literal_columns(
    payload: &str,
) -> Result<Vec<(String, Vec<TabularScalar>)>, serde_json::Error> {
    struct LiteralColumnsVisitor;

    impl<'de> Visitor<'de> for LiteralColumnsVisitor {
        type Value = Vec<(String, Vec<TabularScalar>)>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a tabular column map")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut columns = Vec::new();
            while let Some(name) = map.next_key::<String>()? {
                columns.push((name, map.next_value::<Vec<TabularScalar>>()?));
            }
            Ok(columns)
        }
    }

    let mut deserializer = serde_json::Deserializer::from_str(payload);
    deserializer.deserialize_map(LiteralColumnsVisitor)
}

fn validate_snapshot(
    data_type: &ValueType,
    snapshot: &TabularSnapshot,
) -> Result<(), ConstantValueError> {
    if matches!(data_type, ValueType::DataSeries(_)) && snapshot.columns().len() != 1 {
        return Err(ConstantValueError::Contract(
            TabularContractError::SeriesColumnCount {
                actual: snapshot.columns().len(),
            },
        ));
    }
    if let ValueType::DataSeries(element) = data_type {
        use yss_data_contract::SemanticType;
        let valid = snapshot.columns()[0].values().iter().all(|value| {
            if matches!(value, TabularScalar::Null) || *element.as_ref() == ValueType::Any {
                return true;
            }
            let ValueType::Scalar(semantic) = element.as_ref() else {
                return false;
            };
            match semantic {
                SemanticType::Numeric => matches!(
                    value,
                    TabularScalar::Integer(_)
                        | TabularScalar::Unsigned(_)
                        | TabularScalar::Float64(_)
                ),
                SemanticType::Binary => matches!(value, TabularScalar::Bool(_)),
                SemanticType::Text | SemanticType::Datetime => {
                    matches!(value, TabularScalar::String(_))
                }
                SemanticType::Categorical | SemanticType::Identifier => true,
                // An ordinal sequence requires an explicit level mapping, which a bare literal
                // does not supply. Dataset-backed series retain that mapping in field metadata.
                SemanticType::Ordinal => false,
            }
        });
        if !valid {
            return Err(ConstantValueError::ValueKindMismatch);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("constant '{id}' has an invalid definition")]
pub struct InvalidConstantDefinition {
    pub id: ConstantId,
}

pub fn validate_constant_definitions(
    constants: &std::collections::BTreeMap<ConstantId, GraphConstant>,
) -> Result<(), InvalidConstantDefinition> {
    let mut names = std::collections::BTreeSet::new();
    for (id, constant) in constants {
        let valid = if matches!(
            constant.data_type,
            ValueType::DataFrame | ValueType::DataSeries(_)
        ) {
            constant.data_value == DataValue::Null
                && constant
                    .tabular
                    .as_ref()
                    .is_none_or(|snapshot| validate_snapshot(&constant.data_type, snapshot).is_ok())
        } else {
            !matches!(
                constant.data_type,
                ValueType::Any | ValueType::OneOf(_) | ValueType::Struct(_)
            ) && constant.tabular.is_none()
                && value_matches_type(&constant.data_value, &constant.data_type)
        };
        if id != &constant.id
            || constant.name.trim().is_empty()
            || !names.insert(constant.name.trim())
            || !valid
        {
            return Err(InvalidConstantDefinition { id: *id });
        }
    }
    Ok(())
}

/// Parse editor input once; persisted table constants contain only their snapshot.
/// Validation finishes before any field is changed.
pub fn normalize_constant_value(constant: &mut GraphConstant) -> Result<(), ConstantValueError> {
    if !matches!(
        constant.data_type,
        ValueType::DataFrame | ValueType::DataSeries(_)
    ) {
        if matches!(
            constant.data_type,
            ValueType::Any | ValueType::OneOf(_) | ValueType::Struct(_)
        ) || !value_matches_type(&constant.data_value, &constant.data_type)
        {
            return Err(ConstantValueError::ValueKindMismatch);
        }
        constant.tabular = None;
        return Ok(());
    }
    match &constant.data_value {
        DataValue::Null => {
            if let Some(snapshot) = &constant.tabular {
                validate_snapshot(&constant.data_type, snapshot)?;
            }
        }
        DataValue::String(payload) => {
            let snapshot = parse_literal(payload)?;
            validate_snapshot(&constant.data_type, &snapshot)?;
            constant.tabular = Some(snapshot);
            constant.data_value = DataValue::Null;
        }
        _ => return Err(ConstantValueError::ValueKindMismatch),
    }
    Ok(())
}

fn value_matches_type(value: &DataValue, data_type: &ValueType) -> bool {
    use yss_data_contract::SemanticType;
    match (value, data_type) {
        (DataValue::Null, _) => true,
        (DataValue::Bool(_), ValueType::Scalar(SemanticType::Binary) | ValueType::Any)
        | (DataValue::Integer(_), ValueType::Scalar(SemanticType::Numeric) | ValueType::Any)
        | (
            DataValue::String(_),
            ValueType::Scalar(SemanticType::Text | SemanticType::Datetime) | ValueType::Any,
        ) => true,
        (
            DataValue::Unsigned(_) | DataValue::Decimal(_),
            ValueType::Scalar(SemanticType::Numeric) | ValueType::Any,
        ) => true,
        (
            DataValue::Bool(_) | DataValue::Integer(_) | DataValue::String(_),
            ValueType::Scalar(SemanticType::Categorical | SemanticType::Identifier),
        ) => true,
        (
            DataValue::Unsigned(_) | DataValue::Decimal(_),
            ValueType::Scalar(SemanticType::Categorical | SemanticType::Identifier),
        ) => true,
        (DataValue::List(values), ValueType::Array(element)) => values
            .iter()
            .all(|value| value_matches_type(value, element)),
        (DataValue::List(values), ValueType::Any) => values
            .iter()
            .all(|value| value_matches_type(value, &ValueType::Any)),
        (DataValue::Object(values), ValueType::Object | ValueType::Any) => values
            .values()
            .all(|value| value_matches_type(value, &ValueType::Any)),
        (_, ValueType::OneOf(types)) => types
            .iter()
            .any(|data_type| value_matches_type(value, data_type)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_data_contract::TabularColumnName;

    fn constant(data_type: ValueType, data_value: DataValue) -> GraphConstant {
        GraphConstant {
            id: ConstantId::new(),
            name: "value".into(),
            data_type,
            data_value,
            tabular: None,
            description: String::new(),
            tags: vec![],
        }
    }

    #[test]
    fn compound_defaults_are_empty_instead_of_inventing_user_data() {
        assert_eq!(
            default_value_for(&ValueType::Array(Box::new(ValueType::Scalar(
                yss_data_contract::SemanticType::Text
            )))),
            DataValue::List(Vec::new())
        );
        assert_eq!(
            default_value_for(&ValueType::Object),
            DataValue::Object(std::collections::BTreeMap::new())
        );
        assert_eq!(
            default_value_for(&ValueType::OneOf(vec![
                ValueType::Scalar(yss_data_contract::SemanticType::Binary),
                ValueType::Scalar(yss_data_contract::SemanticType::Numeric)
            ])),
            DataValue::Bool(false)
        );
        assert_eq!(
            default_value_for(&ValueType::OneOf(Vec::new())),
            DataValue::Null
        );
    }

    #[test]
    fn normalization_and_copy_preserve_the_snapshot_without_a_handle() {
        let mut constant = constant(
            ValueType::DataFrame,
            DataValue::String(r#"{"value":[1,2]}"#.into()),
        );
        normalize_constant_value(&mut constant).unwrap();
        assert_eq!(constant.data_value, DataValue::Null);
        let normalized = constant.clone();
        normalize_constant_value(&mut constant).unwrap();
        assert_eq!(constant, normalized);
        let copy = constant.copy_with_id(ConstantId::new());
        assert_ne!(copy.id, constant.id);
        assert_eq!(copy.data_value, DataValue::Null);
        assert_eq!(copy.tabular, constant.tabular);
    }

    #[test]
    fn series_literals_are_checked_against_the_declared_element_semantic() {
        let mut constant = constant(
            ValueType::DataSeries(Box::new(ValueType::Scalar(
                yss_data_contract::SemanticType::Numeric,
            ))),
            DataValue::String(r#"{"value":["text"]}"#.into()),
        );
        let before = constant.clone();
        assert_eq!(
            normalize_constant_value(&mut constant),
            Err(ConstantValueError::ValueKindMismatch)
        );
        assert_eq!(constant, before);
    }

    #[test]
    fn invalid_tabular_payload_leaves_value_and_snapshot_unchanged() {
        let mut constant = constant(
            ValueType::DataFrame,
            DataValue::String(r#"{"value":[1]}"#.into()),
        );
        normalize_constant_value(&mut constant).expect("initial value");
        let before = constant.clone();
        constant.data_value = DataValue::String(r#"{"value":[{"nested":true}]}"#.into());

        assert!(matches!(
            normalize_constant_value(&mut constant),
            Err(ConstantValueError::UnsupportedCell { .. })
        ));
        assert_eq!(constant.tabular, before.tabular);
        assert_eq!(
            constant.data_value,
            DataValue::String(r#"{"value":[{"nested":true}]}"#.into())
        );
    }

    #[test]
    fn duplicate_column_contract_error_is_preserved() {
        let mut constant = constant(
            ValueType::DataFrame,
            DataValue::String(r#"{"value":[1],"value":[2]}"#.into()),
        );

        assert_eq!(
            normalize_constant_value(&mut constant),
            Err(ConstantValueError::Contract(
                TabularContractError::DuplicateColumnName {
                    column: TabularColumnName::try_from("value").expect("valid test name"),
                },
            ))
        );
    }
}
