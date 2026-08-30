//! Pure variable-value defaults and tabular snapshot normalization.
//!
//! This crate owns the relationship between persisted variable types, values,
//! stable tabular handles, and embedded tabular snapshots. It does not own
//! project state, persistence, mutation authority, or backend materialization.

use serde::Deserializer as _;
use serde::de::{MapAccess, Visitor};
use serde_json::Value;
use std::fmt;
use yss_data_contract::{DataType, DataValue};
use yss_tabular_contract::{TabularColumn, TabularContractError, TabularScalar, TabularSnapshot};
use yss_variable_contract::{VariableId, VariableInstance};

const VARIABLE_HANDLE_PREFIX: &str = "var:";

/// Returns an inert value suitable for a variable whose type has just changed.
///
/// Compound defaults are deliberately empty. Populating arrays or objects with
/// sample data would invent user data and can violate the declared element type.
pub fn default_value_for(data_type: &DataType) -> DataValue {
    match data_type {
        DataType::Boolean => DataValue::Boolean(false),
        DataType::Int64 => DataValue::Int64(0),
        DataType::Float64 => DataValue::Float64(0.0),
        DataType::String
        | DataType::Date
        | DataType::Datetime
        | DataType::Time
        | DataType::Categorical => DataValue::String(String::new()),
        DataType::Array(_) => DataValue::Array(Vec::new()),
        DataType::Object => DataValue::Object(std::collections::HashMap::new()),
        DataType::OneOf(types) => types.first().map_or(DataValue::Null, default_value_for),
        DataType::Any | DataType::DataFrame | DataType::DataSeries(_) | DataType::Struct(_) => {
            DataValue::Null
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VariableTabularNormalizationError {
    #[error("tabular variable value kind is invalid")]
    ValueKindMismatch,
    #[error("tabular variable handle belongs to another variable")]
    ForeignVariableHandle,
    #[error("tabular variable handle has no embedded snapshot")]
    MissingSnapshot,
    #[error("tabular variable JSON is invalid")]
    InvalidJson,
    #[error("tabular variable JSON must be a column map")]
    ExpectedColumnMap,
    #[error("tabular variable column must be an array")]
    ColumnNotArray {
        column: yss_tabular_contract::TabularColumnName,
    },
    #[error("tabular variable cell must be a scalar")]
    UnsupportedCell {
        column: yss_tabular_contract::TabularColumnName,
        row: usize,
    },
    #[error("tabular variable contract is invalid")]
    Contract(TabularContractError),
}

pub fn variable_handle(id: &VariableId) -> String {
    format!("{VARIABLE_HANDLE_PREFIX}{id}")
}

fn is_variable_handle(value: &str) -> bool {
    value.starts_with(VARIABLE_HANDLE_PREFIX)
}

fn parse_literal(payload: &str) -> Result<TabularSnapshot, VariableTabularNormalizationError> {
    let parsed: Value = serde_json::from_str(payload)
        .map_err(|_| VariableTabularNormalizationError::InvalidJson)?;
    let Value::Object(columns) = &parsed else {
        return Err(VariableTabularNormalizationError::ExpectedColumnMap);
    };
    for (name, values) in columns {
        let column = yss_tabular_contract::TabularColumnName::try_from(name.as_str())
            .map_err(VariableTabularNormalizationError::Contract)?;
        let Some(values) = values.as_array() else {
            return Err(VariableTabularNormalizationError::ColumnNotArray { column });
        };
        if let Some(row) = values.iter().position(|value| {
            !matches!(
                value,
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
            )
        }) {
            return Err(VariableTabularNormalizationError::UnsupportedCell { column, row });
        }
    }

    let columns = deserialize_literal_columns(payload).map_err(|error| match error.classify() {
        serde_json::error::Category::Io
        | serde_json::error::Category::Syntax
        | serde_json::error::Category::Data
        | serde_json::error::Category::Eof => VariableTabularNormalizationError::InvalidJson,
    })?;

    TabularSnapshot::try_from_columns(
        columns
            .into_iter()
            .map(|(name, values)| {
                yss_tabular_contract::TabularColumnName::try_from(name.as_str())
                    .map(|name| TabularColumn::new(name, values.into_boxed_slice()))
                    .map_err(VariableTabularNormalizationError::Contract)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice(),
    )
    .map_err(VariableTabularNormalizationError::Contract)
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

enum TabularInput {
    Clear,
    Unchanged,
    Snapshot(TabularSnapshot),
}

fn classify_payload(
    payload: &str,
    canonical_handle: &str,
) -> Result<TabularInput, VariableTabularNormalizationError> {
    if payload == canonical_handle {
        return Ok(TabularInput::Unchanged);
    }
    if is_variable_handle(payload) {
        return Err(VariableTabularNormalizationError::ForeignVariableHandle);
    }
    if !payload.trim_start().starts_with(['{', '[']) {
        return Err(VariableTabularNormalizationError::ValueKindMismatch);
    }
    Ok(TabularInput::Snapshot(parse_literal(payload)?))
}

fn ingest(variable: &VariableInstance) -> Result<TabularInput, VariableTabularNormalizationError> {
    let canonical_handle = variable_handle(&variable.id);
    match (&variable.data_type, &variable.data_value) {
        (DataType::DataFrame, DataValue::Null) | (DataType::DataSeries(_), DataValue::Null) => {
            Ok(TabularInput::Clear)
        }
        (DataType::DataFrame, DataValue::DataFrame(payload)) => {
            classify_payload(payload, &canonical_handle)
        }
        (DataType::DataSeries(_), DataValue::DataSeries(value)) => {
            classify_payload(&value.id, &canonical_handle)
        }
        _ => Err(VariableTabularNormalizationError::ValueKindMismatch),
    }
}

fn validate_snapshot(
    data_type: &DataType,
    snapshot: &TabularSnapshot,
) -> Result<(), VariableTabularNormalizationError> {
    if matches!(data_type, DataType::DataSeries(_)) && snapshot.columns().len() != 1 {
        return Err(VariableTabularNormalizationError::Contract(
            TabularContractError::SeriesColumnCount {
                actual: snapshot.columns().len(),
            },
        ));
    }
    Ok(())
}

/// Normalizes a variable's tabular literal into an embedded snapshot and stable handle.
///
/// The update is atomic: on error, neither the value nor the snapshot is changed.
pub fn normalize_variable_tabular(
    variable: &mut VariableInstance,
) -> Result<(), VariableTabularNormalizationError> {
    if !matches!(
        variable.data_type,
        DataType::DataFrame | DataType::DataSeries(_)
    ) {
        variable.tabular = None;
        return Ok(());
    }

    let next_tabular = match ingest(variable)? {
        TabularInput::Clear => None,
        TabularInput::Unchanged => {
            let snapshot = variable
                .tabular
                .as_ref()
                .ok_or(VariableTabularNormalizationError::MissingSnapshot)?;
            validate_snapshot(&variable.data_type, snapshot)?;
            Some(snapshot.clone())
        }
        TabularInput::Snapshot(snapshot) => {
            validate_snapshot(&variable.data_type, &snapshot)?;
            Some(snapshot)
        }
    };
    let next_data_value = if next_tabular.is_none() {
        variable.data_value.clone()
    } else {
        match (&variable.data_type, &variable.data_value) {
            (DataType::DataFrame, DataValue::DataFrame(_)) => {
                DataValue::DataFrame(variable_handle(&variable.id))
            }
            (DataType::DataSeries(_), DataValue::DataSeries(value)) => {
                let mut value = value.clone();
                value.id = variable_handle(&variable.id);
                DataValue::DataSeries(value)
            }
            _ => return Err(VariableTabularNormalizationError::ValueKindMismatch),
        }
    };

    variable.tabular = next_tabular;
    variable.data_value = next_data_value;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_data_contract::DataSeriesValue;
    use yss_tabular_contract::TabularColumnName;
    use yss_variable_contract::VariableScope;

    fn variable(data_type: DataType, data_value: DataValue) -> VariableInstance {
        VariableInstance {
            id: VariableId::new(),
            name: "value".into(),
            data_type,
            data_value,
            tabular: None,
            description: String::new(),
            scope: VariableScope::Global,
            tags: vec![],
        }
    }

    #[test]
    fn compound_defaults_are_empty_instead_of_inventing_user_data() {
        assert_eq!(
            default_value_for(&DataType::Array(Box::new(DataType::String))),
            DataValue::Array(Vec::new())
        );
        assert_eq!(
            default_value_for(&DataType::Object),
            DataValue::Object(std::collections::HashMap::new())
        );
        assert_eq!(
            default_value_for(&DataType::OneOf(vec![DataType::Boolean, DataType::Int64])),
            DataValue::Boolean(false)
        );
        assert_eq!(
            default_value_for(&DataType::OneOf(Vec::new())),
            DataValue::Null
        );
    }

    #[test]
    fn normalize_enforces_current_variable_canonical_handle() {
        let mut variable = variable(
            DataType::DataFrame,
            DataValue::DataFrame(r#"{"value":[1,2]}"#.into()),
        );

        normalize_variable_tabular(&mut variable).expect("valid tabular variable");
        assert_eq!(
            variable.data_value,
            DataValue::DataFrame(variable_handle(&variable.id))
        );
        let snapshot = variable.tabular.clone();

        normalize_variable_tabular(&mut variable).expect("canonical handle is unchanged");
        assert_eq!(variable.tabular, snapshot);

        let foreign_handle = variable_handle(&VariableId::new());
        variable.data_value = DataValue::DataFrame(foreign_handle.clone());

        assert_eq!(
            normalize_variable_tabular(&mut variable),
            Err(VariableTabularNormalizationError::ForeignVariableHandle)
        );
        assert_eq!(variable.data_value, DataValue::DataFrame(foreign_handle));
        assert_eq!(variable.tabular, snapshot);
    }

    #[test]
    fn canonical_handle_without_snapshot_is_rejected_atomically() {
        let mut variable = variable(DataType::DataFrame, DataValue::Null);
        variable.data_value = DataValue::DataFrame(variable_handle(&variable.id));
        let before = variable.clone();

        assert_eq!(
            normalize_variable_tabular(&mut variable),
            Err(VariableTabularNormalizationError::MissingSnapshot)
        );
        assert_eq!(variable, before);
    }

    #[test]
    fn data_series_normalization_preserves_non_handle_metadata() {
        let mut variable = variable(
            DataType::DataSeries(Box::new(DataType::Int64)),
            DataValue::DataSeries(DataSeriesValue::with_element_type(
                r#"{"value":[1,2]}"#,
                DataType::Int64,
            )),
        );

        normalize_variable_tabular(&mut variable).expect("valid data series literal");
        let DataValue::DataSeries(value) = &variable.data_value else {
            panic!("normalization must retain the data-series value kind");
        };
        assert_eq!(value.id, variable_handle(&variable.id));
        assert_eq!(value.element_type, Some(DataType::Int64));
    }

    #[test]
    fn invalid_tabular_payload_leaves_value_and_snapshot_unchanged() {
        let mut variable = variable(
            DataType::DataFrame,
            DataValue::DataFrame(r#"{"value":[1]}"#.into()),
        );
        normalize_variable_tabular(&mut variable).expect("initial value");
        let before = variable.clone();
        variable.data_value = DataValue::DataFrame(r#"{"value":[{"nested":true}]}"#.into());

        assert!(matches!(
            normalize_variable_tabular(&mut variable),
            Err(VariableTabularNormalizationError::UnsupportedCell { .. })
        ));
        assert_eq!(variable.tabular, before.tabular);
        assert_eq!(
            variable.data_value,
            DataValue::DataFrame(r#"{"value":[{"nested":true}]}"#.into())
        );
    }

    #[test]
    fn duplicate_column_contract_error_is_preserved() {
        let mut variable = variable(
            DataType::DataFrame,
            DataValue::DataFrame(r#"{"value":[1],"value":[2]}"#.into()),
        );

        assert_eq!(
            normalize_variable_tabular(&mut variable),
            Err(VariableTabularNormalizationError::Contract(
                TabularContractError::DuplicateColumnName {
                    column: TabularColumnName::try_from("value").expect("valid test name"),
                },
            ))
        );
    }
}
