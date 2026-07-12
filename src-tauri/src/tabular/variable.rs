use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use crate::project::ProjectStore;
use crate::variable::{VariableId, VariableInstance};

use super::catalog::build_variable_cache_entry;
use super::snapshot::{TabularSnapshot, is_json_literal};
use super::{variable_handle, variable_handle_str};

/// 从前端/API 提交的值 ingest 为 tabular snapshot（JSON 列式对象）。
pub fn ingest_tabular_input(
    data_type: &DataType,
    data_value: &DataValue,
) -> Result<Option<TabularSnapshot>, String> {
    match data_type {
        DataType::DataFrame => match data_value {
            DataValue::Null => Ok(None),
            DataValue::DataFrame(raw) if is_json_literal(raw) => {
                Ok(Some(TabularSnapshot::from_json(raw)?))
            }
            DataValue::DataFrame(raw) if super::r#ref::is_variable_handle(raw) => Ok(None),
            DataValue::DataFrame(_) => Ok(None),
            _ => Ok(None),
        },
        DataType::DataSeries(_) => match data_value {
            DataValue::Null => Ok(None),
            DataValue::DataSeries(dsv) if is_json_literal(&dsv.id) => {
                let snapshot = TabularSnapshot::from_json(&dsv.id)?;
                if snapshot.width() != 1 {
                    return Err(format!(
                        "DataSeries variable: expected exactly one column, got {}",
                        snapshot.width()
                    ));
                }
                Ok(Some(snapshot))
            }
            DataValue::DataSeries(dsv) if super::r#ref::is_variable_handle(&dsv.id) => Ok(None),
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

/// 规范化变量：结构化 tabular + 稳定 handle（`var:{id}`）。
pub fn normalize_variable_tabular(var: &mut VariableInstance) -> Result<(), String> {
    if !matches!(var.data_type, DataType::DataFrame | DataType::DataSeries(_)) {
        var.tabular = None;
        return Ok(());
    }

    let ingested = ingest_tabular_input(&var.data_type, &var.data_value)?;
    if let Some(snapshot) = ingested {
        var.tabular = Some(snapshot);
    }

    match (&var.data_type, &var.data_value) {
        (DataType::DataFrame, DataValue::Null) => {
            var.tabular = None;
        }
        (DataType::DataSeries(_), DataValue::Null) => {
            var.tabular = None;
        }
        _ => {}
    }

    if let Some(snapshot) = &var.tabular {
        if var.data_type == DataType::DataFrame {
            var.data_value = DataValue::DataFrame(variable_handle(&var.id));
        } else if matches!(var.data_type, DataType::DataSeries(_)) {
            if snapshot.width() != 1 {
                return Err(format!(
                    "DataSeries variable: expected exactly one column, got {}",
                    snapshot.width()
                ));
            }
            var.data_value = DataValue::DataSeries(DataSeriesValue::new(variable_handle(&var.id)));
        }
    }

    Ok(())
}

pub fn sync_variable_cache(store: &mut ProjectStore, var: &VariableInstance) -> Result<(), String> {
    let handle = variable_handle(&var.id);
    if let Some(snapshot) = &var.tabular {
        let entry = build_variable_cache_entry(snapshot)?;
        store.variable_tabular.insert(handle, entry);
    } else {
        store.variable_tabular.remove(&handle);
    }
    Ok(())
}

pub fn remove_variable_cache(store: &mut ProjectStore, variable_id: &VariableId) {
    store
        .variable_tabular
        .remove(&variable_handle_str(&variable_id.to_string()));
}

/// API/DTO 展示：tabular 变量以 JSON 列式对象返回给前端编辑器。
pub fn display_data_value(var: &VariableInstance) -> DataValue {
    if let Some(snapshot) = &var.tabular {
        let json = snapshot.to_json().unwrap_or_else(|_| "{}".to_string());
        return match var.data_type {
            DataType::DataFrame => DataValue::DataFrame(json),
            DataType::DataSeries(_) => DataValue::DataSeries(DataSeriesValue::new(json)),
            _ => var.data_value.clone(),
        };
    }
    var.data_value.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variable::VariableScope;

    #[test]
    fn normalizes_dataframe_variable_to_stable_handle() {
        let mut var = VariableInstance {
            id: VariableId::new(),
            name: "df".to_string(),
            data_type: DataType::DataFrame,
            data_value: DataValue::DataFrame(r#"{"a":[1,2]}"#.to_string()),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Global,
            tags: vec![],
        };
        normalize_variable_tabular(&mut var).unwrap();
        assert!(var.tabular.is_some());
        assert_eq!(
            var.data_value,
            DataValue::DataFrame(variable_handle(&var.id))
        );
    }
}
