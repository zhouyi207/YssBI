use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use crate::variable::VariableInstance;

pub mod dataframe_io;
mod r#ref;
pub mod snapshot;

pub use r#ref::variable_handle;
pub use snapshot::TabularSnapshot;

enum TabularInput {
    Clear,
    Unchanged,
    Snapshot(TabularSnapshot),
}

fn classify_tabular_payload(
    payload: &str,
    canonical_handle: &str,
    kind: &str,
) -> Result<TabularInput, String> {
    if payload == canonical_handle {
        return Ok(TabularInput::Unchanged);
    }
    if r#ref::is_variable_handle(payload) {
        return Err(format!(
            "{kind} variable handle does not match the variable ID"
        ));
    }
    if !snapshot::is_json_literal(payload) {
        return Err(format!(
            "{kind} variable value must be tabular JSON or its canonical handle"
        ));
    }
    Ok(TabularInput::Snapshot(TabularSnapshot::from_json(payload)?))
}

fn ingest_tabular_input(variable: &VariableInstance) -> Result<TabularInput, String> {
    let canonical_handle = variable_handle(&variable.id);
    match (&variable.data_type, &variable.data_value) {
        (DataType::DataFrame, DataValue::Null) | (DataType::DataSeries(_), DataValue::Null) => {
            Ok(TabularInput::Clear)
        }
        (DataType::DataFrame, DataValue::DataFrame(payload)) => {
            classify_tabular_payload(payload, &canonical_handle, "DataFrame")
        }
        (DataType::DataSeries(_), DataValue::DataSeries(value)) => {
            classify_tabular_payload(&value.id, &canonical_handle, "DataSeries")
        }
        (DataType::DataFrame, _) => Err(
            "DataFrame variable value must be Null, DataFrame JSON, or its canonical handle".into(),
        ),
        (DataType::DataSeries(_), _) => Err(
            "DataSeries variable value must be Null, DataSeries JSON, or its canonical handle"
                .into(),
        ),
        _ => unreachable!("tabular ingestion requires a tabular data type"),
    }
}

fn validate_snapshot(data_type: &DataType, snapshot: &TabularSnapshot) -> Result<(), String> {
    if matches!(data_type, DataType::DataSeries(_)) && snapshot.width() != 1 {
        return Err(format!(
            "DataSeries variable: expected exactly one column, got {}",
            snapshot.width()
        ));
    }
    snapshot.to_dataframe()?;
    Ok(())
}

pub fn normalize_variable_tabular(variable: &mut VariableInstance) -> Result<(), String> {
    if !matches!(
        variable.data_type,
        DataType::DataFrame | DataType::DataSeries(_)
    ) {
        variable.tabular = None;
        return Ok(());
    }

    match ingest_tabular_input(variable)? {
        TabularInput::Clear => variable.tabular = None,
        TabularInput::Unchanged => {
            if let Some(snapshot) = &variable.tabular {
                validate_snapshot(&variable.data_type, snapshot)?;
            }
        }
        TabularInput::Snapshot(snapshot) => {
            validate_snapshot(&variable.data_type, &snapshot)?;
            variable.tabular = Some(snapshot);
        }
    }

    if variable.tabular.is_some() {
        let handle = variable_handle(&variable.id);
        variable.data_value = match variable.data_type {
            DataType::DataFrame => DataValue::DataFrame(handle),
            DataType::DataSeries(_) => DataValue::DataSeries(DataSeriesValue::new(handle)),
            _ => unreachable!("tabular normalization requires a tabular data type"),
        };
    }

    Ok(())
}

pub fn display_data_value(variable: &VariableInstance) -> DataValue {
    if let Some(snapshot) = &variable.tabular {
        let json = snapshot.to_json().unwrap_or_else(|_| "{}".to_string());
        return match variable.data_type {
            DataType::DataFrame => DataValue::DataFrame(json),
            DataType::DataSeries(_) => DataValue::DataSeries(DataSeriesValue::new(json)),
            _ => variable.data_value.clone(),
        };
    }
    variable.data_value.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variable::{VariableId, VariableScope};

    #[test]
    fn normalize_enforces_current_variable_canonical_handle() {
        let mut variable = VariableInstance {
            id: VariableId::new(),
            name: "table".into(),
            data_type: DataType::DataFrame,
            data_value: DataValue::DataFrame(r#"{"value":[1,2]}"#.into()),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Global,
            tags: vec![],
        };

        normalize_variable_tabular(&mut variable).unwrap();
        assert_eq!(
            variable.data_value,
            DataValue::DataFrame(variable_handle(&variable.id))
        );
        let snapshot = variable.tabular.clone();

        normalize_variable_tabular(&mut variable).unwrap();
        assert_eq!(variable.tabular, snapshot);

        let foreign_handle = variable_handle(&VariableId::new());
        variable.data_value = DataValue::DataFrame(foreign_handle.clone());

        assert!(normalize_variable_tabular(&mut variable).is_err());
        assert_eq!(variable.data_value, DataValue::DataFrame(foreign_handle));
        assert_eq!(variable.tabular, snapshot);
    }
}
