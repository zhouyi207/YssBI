//! 列式 tabular 快照：`{ "col_a": [1,2], "col_b": [3,4] }`

use crate::database::json_to_anyvalue;
use polars::prelude::{AnyValue, Column, DataFrame, DataType as PDataType, PlSmallStr, Series};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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
            return Err(
                "Tabular JSON: expected object mapping column names to value arrays".to_string(),
            );
        };
        let mut columns = BTreeMap::new();
        for (name, value) in map {
            let Value::Array(values) = value else {
                return Err(format!(
                    "Tabular JSON: column '{name}' must be an array of values"
                ));
            };
            columns.insert(name, values);
        }
        Ok(Self { columns })
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(&self.columns).map_err(|e| format!("Tabular JSON encode error: {e}"))
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
    let any_values: Result<Vec<AnyValue<'static>>, String> =
        values.iter().map(|v| json_to_anyvalue(v, &dtype)).collect();
    let any_values = any_values?;
    Series::from_any_values(PlSmallStr::from(name), &any_values, false)
        .map_err(|e| format!("Failed to build Series '{name}': {e}"))
}

pub fn is_json_literal(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('[') || t.starts_with('{')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_column_map_dataframe() {
        let snapshot = TabularSnapshot::from_json(r#"{"a":[1,2],"b":[3,4]}"#).unwrap();
        let dataframe = snapshot.to_dataframe().unwrap();
        assert_eq!(dataframe.height(), 2);
        assert_eq!(dataframe.width(), 2);
    }
}
