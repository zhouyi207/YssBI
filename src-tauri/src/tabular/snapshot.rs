//! 列式 tabular 快照：`{ "col_a": [1,2], "col_b": [3,4] }`

use crate::database::polars_dtype_to_data_type;
use crate::graph::node::{ColumnSchema, DataSchema};
use polars::prelude::{AnyValue, Column, DataFrame, DataType as PDataType, PlSmallStr, Series};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use yss_sci::database::json_to_anyvalue;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TabularSnapshot {
    #[serde(default)]
    pub columns: BTreeMap<String, Vec<Value>>,
}

impl TabularSnapshot {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let parsed: Value =
            serde_json::from_str(json).map_err(|e| format!("Tabular JSON parse error: {e}"))?;
        let Value::Object(map) = parsed else {
            return Err("Tabular JSON: expected object mapping column names to value arrays".to_string());
        };
        let mut columns = BTreeMap::new();
        for (name, value) in map {
            let Value::Array(values) = value else {
                return Err(format!("Tabular JSON: column '{name}' must be an array of values"));
            };
            columns.insert(name, values);
        }
        Ok(Self { columns })
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(&self.columns).map_err(|e| format!("Tabular JSON encode error: {e}"))
    }

    pub fn to_json_pretty(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.columns)
            .map_err(|e| format!("Tabular JSON encode error: {e}"))
    }

    pub fn column_names(&self) -> Vec<String> {
        self.columns.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn width(&self) -> usize {
        self.columns.len()
    }

    pub fn height(&self) -> Result<usize, String> {
        let mut heights = self.columns.values().map(|v| v.len());
        let Some(first) = heights.next() else {
            return Ok(0);
        };
        if heights.any(|h| h != first) {
            return Err("Tabular snapshot: all columns must have the same length".to_string());
        }
        Ok(first)
    }

    pub fn to_dataframe(&self) -> Result<DataFrame, String> {
        if self.columns.is_empty() {
            return DataFrame::new(0, Vec::<Column>::new())
                .map_err(|e| format!("Failed to build empty DataFrame: {e}"));
        }
        let height = self.height()?;
        let mut series_vec = Vec::with_capacity(self.columns.len());
        for (name, values) in &self.columns {
            series_vec.push(values_to_series(name, values)?);
        }
        let columns: Vec<Column> = series_vec.into_iter().map(Column::from).collect();
        DataFrame::new(height, columns).map_err(|e| format!("Failed to build DataFrame: {e}"))
    }

    pub fn to_schema(&self) -> Result<DataSchema, String> {
        let df = self.to_dataframe()?;
        let columns = df
            .columns()
            .iter()
            .map(|series| ColumnSchema {
                name: series.name().to_string(),
                data_type: polars_dtype_to_data_type(series.dtype()),
            })
            .collect();
        Ok(DataSchema { columns })
    }
}

fn infer_polars_dtype(values: &[Value]) -> PDataType {
    let mut saw_int = false;
    let mut saw_float = false;
    let mut saw_bool = false;
    let mut saw_string = false;
    let mut non_null = 0usize;

    for v in values {
        if v.is_null() {
            continue;
        }
        non_null += 1;
        match v {
            Value::Bool(_) => saw_bool = true,
            Value::Number(n) => {
                if n.as_i64().is_some() && !n.to_string().contains('.') {
                    saw_int = true;
                } else {
                    saw_float = true;
                }
            }
            Value::String(_) => saw_string = true,
            _ => saw_string = true,
        }
    }

    if non_null == 0 {
        return PDataType::String;
    }
    if saw_string || (saw_bool as u8 + saw_int as u8 + saw_float as u8) > 1 {
        return PDataType::String;
    }
    if saw_bool {
        return PDataType::Boolean;
    }
    if saw_float {
        return PDataType::Float64;
    }
    if saw_int {
        return PDataType::Int64;
    }
    PDataType::String
}

fn values_to_series(name: &str, values: &[Value]) -> Result<Series, String> {
    let dtype = infer_polars_dtype(values);
    let any_values: Result<Vec<AnyValue<'static>>, String> = values
        .iter()
        .map(|v| json_to_anyvalue(v, &dtype))
        .collect();
    let any_values = any_values?;
    Series::from_any_values(PlSmallStr::from(name), &any_values, false)
        .map_err(|e| format!("Failed to build Series '{name}': {e}"))
}

pub fn is_json_literal(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('[') || t.starts_with('{')
}

pub fn dataframe_from_json(json: &str) -> Result<DataFrame, String> {
    TabularSnapshot::from_json(json)?.to_dataframe()
}

pub fn series_from_json(json: &str) -> Result<Series, String> {
    let snapshot = TabularSnapshot::from_json(json)?;
    if snapshot.width() != 1 {
        return Err(format!(
            "DataSeries JSON: expected exactly one column, got {}",
            snapshot.width()
        ));
    }
    let df = snapshot.to_dataframe()?;
    df.select_at_idx(0)
        .ok_or_else(|| "DataSeries JSON: failed to read column".to_string())
        .map(|col| col.clone().take_materialized_series())
}

pub fn dataframe_schema_from_json(json: &str) -> Result<DataSchema, String> {
    TabularSnapshot::from_json(json)?.to_schema()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::DataType;

    #[test]
    fn parses_column_map_dataframe() {
        let json = r#"{"a":[1,2],"b":[3,4]}"#;
        let df = dataframe_from_json(json).unwrap();
        assert_eq!(df.height(), 2);
        assert_eq!(df.width(), 2);
    }

    #[test]
    fn infers_schema_from_column_map() {
        let json = r#"{"a":[1,2],"b":[1.5,2.5]}"#;
        let schema = dataframe_schema_from_json(json).unwrap();
        assert_eq!(schema.columns.len(), 2);
        assert_eq!(schema.columns[0].data_type, DataType::Int64);
        assert_eq!(schema.columns[1].data_type, DataType::Float64);
    }

    #[test]
    fn parses_dataseries_single_column() {
        let json = r#"{"price":[1,2,3]}"#;
        let s = series_from_json(json).unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s.name(), "price");
    }
}
