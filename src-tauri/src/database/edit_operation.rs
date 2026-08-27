use chrono::{Datelike, NaiveDate};
use polars::chunked_array::cast::CastOptions;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

use crate::backend_adapters::tabular::polars::json_to_anyvalue;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EditOperation {
    EditCell {
        row: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        row_id: Option<i64>,
        col: String,
        old_value: serde_json::Value,
        new_value: serde_json::Value,
    },
    AddRow {
        index: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        row_id: Option<i64>,
    },
    DeleteRow {
        index: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        row_id: Option<i64>,
        data: Vec<serde_json::Value>,
    },
    AddColumn {
        name: String,
        dtype: String,
    },
    DeleteColumn {
        name: String,
        dtype: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        row_ids: Vec<i64>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        row_fingerprints: Vec<u64>,
        data: Vec<serde_json::Value>,
    },
    RenameColumn {
        old_name: String,
        new_name: String,
    },
    CastColumn {
        col: String,
        old_data: Vec<serde_json::Value>,
        old_dtype: String,
        new_dtype: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_modified: bool,
    pub undo_count: usize,
    pub redo_count: usize,
}

#[derive(Debug, Clone)]
pub struct EditHistory {
    undo_stack: Vec<EditOperation>,
    redo_stack: Vec<EditOperation>,
}

impl EditHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn push(&mut self, op: EditOperation) {
        self.undo_stack.push(op);
        self.redo_stack.clear();
    }

    pub fn pop_undo(&mut self) -> Option<EditOperation> {
        self.undo_stack.pop()
    }

    pub fn push_redo(&mut self, op: EditOperation) {
        self.redo_stack.push(op);
    }

    pub fn pop_redo(&mut self) -> Option<EditOperation> {
        self.redo_stack.pop()
    }

    pub fn push_undo(&mut self, op: EditOperation) {
        self.undo_stack.push(op);
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn state(&self) -> EditState {
        EditState {
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            is_modified: !self.undo_stack.is_empty(),
            undo_count: self.undo_stack.len(),
            redo_count: self.redo_stack.len(),
        }
    }
}

pub fn anyvalue_to_json(val: AnyValue<'_>) -> serde_json::Value {
    use polars::prelude::TimeUnit;
    match val {
        AnyValue::Null => serde_json::Value::Null,
        AnyValue::Boolean(b) => serde_json::Value::Bool(b),
        AnyValue::String(s) => serde_json::Value::String(s.to_string()),
        AnyValue::StringOwned(s) => serde_json::Value::String(s.to_string()),
        AnyValue::Int8(v) => serde_json::json!(v),
        AnyValue::Int16(v) => serde_json::json!(v),
        AnyValue::Int32(v) => serde_json::json!(v),
        AnyValue::Int64(v) => serde_json::json!(v),
        AnyValue::UInt8(v) => serde_json::json!(v),
        AnyValue::UInt16(v) => serde_json::json!(v),
        AnyValue::UInt32(v) => serde_json::json!(v),
        AnyValue::UInt64(v) => serde_json::json!(v),
        AnyValue::Float32(v) => serde_json::Number::from_f64(v as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        AnyValue::Float64(v) => serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        AnyValue::Date(days) => {
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)
                .unwrap()
                .num_days_from_ce();
            NaiveDate::from_num_days_from_ce_opt(epoch + days as i32)
                .map(|d| serde_json::Value::String(d.format("%Y-%m-%d").to_string()))
                .unwrap_or_else(|| serde_json::Value::String(days.to_string()))
        }
        AnyValue::Datetime(ts, unit, _) | AnyValue::DatetimeOwned(ts, unit, _) => {
            let (secs, nsecs) = match unit {
                TimeUnit::Nanoseconds => ((ts / 1_000_000_000) as i64, (ts % 1_000_000_000) as u32),
                TimeUnit::Microseconds => {
                    ((ts / 1_000_000) as i64, ((ts % 1_000_000) * 1000) as u32)
                }
                TimeUnit::Milliseconds => ((ts / 1000) as i64, ((ts % 1000) * 1_000_000) as u32),
            };
            chrono::DateTime::from_timestamp(secs, nsecs)
                .map(|dt| {
                    let s = dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
                    let s = s.trim_end_matches('0').trim_end_matches('.');
                    serde_json::Value::String(s.to_string())
                })
                .unwrap_or_else(|| serde_json::Value::String(ts.to_string()))
        }
        AnyValue::Time(ns) => {
            let secs = (ns / 1_000_000_000) as u32;
            serde_json::Value::String(format!(
                "{:02}:{:02}:{:02}",
                secs / 3600,
                (secs % 3600) / 60,
                secs % 60
            ))
        }
        _ => serde_json::Value::String(format!("{}", val)),
    }
}

fn set_cell(
    df: &mut DataFrame,
    row: usize,
    col: &str,
    val: &serde_json::Value,
) -> Result<(), String> {
    let col_idx = df
        .get_column_index(col)
        .ok_or_else(|| format!("Column '{}' not found", col))?;
    let dtype = df.columns()[col_idx].dtype().clone();
    let av = json_to_anyvalue(val, &dtype).map_err(|error| error.to_string())?;

    let series = df.columns()[col_idx].as_materialized_series();
    let mut new_vec: Vec<AnyValue<'static>> = Vec::with_capacity(series.len());
    for i in 0..series.len() {
        if i == row {
            new_vec.push(av.clone());
        } else {
            let v = series.get(i).map_err(|e| e.to_string())?;
            new_vec.push(owned_anyvalue(v));
        }
    }
    let new_series = Series::from_any_values(series.name().clone(), &new_vec, false)
        .map_err(|e| e.to_string())?;
    let new_col = Column::from(new_series);
    df.replace_column(col_idx, new_col)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn owned_anyvalue(v: AnyValue<'_>) -> AnyValue<'static> {
    match v {
        AnyValue::Null => AnyValue::Null,
        AnyValue::Boolean(b) => AnyValue::Boolean(b),
        AnyValue::Int8(v) => AnyValue::Int8(v),
        AnyValue::Int16(v) => AnyValue::Int16(v),
        AnyValue::Int32(v) => AnyValue::Int32(v),
        AnyValue::Int64(v) => AnyValue::Int64(v),
        AnyValue::UInt8(v) => AnyValue::UInt8(v),
        AnyValue::UInt16(v) => AnyValue::UInt16(v),
        AnyValue::UInt32(v) => AnyValue::UInt32(v),
        AnyValue::UInt64(v) => AnyValue::UInt64(v),
        AnyValue::Float32(v) => AnyValue::Float32(v),
        AnyValue::Float64(v) => AnyValue::Float64(v),
        AnyValue::String(s) => AnyValue::StringOwned(s.to_string().into()),
        AnyValue::StringOwned(s) => AnyValue::StringOwned(s),
        _ => AnyValue::StringOwned(format!("{}", v).into()),
    }
}

pub fn dtype_from_string(s: &str) -> Result<DataType, String> {
    let dtype = match s.to_lowercase().as_str() {
        "int8" | "i8" => DataType::Int8,
        "int16" | "i16" => DataType::Int16,
        "int32" | "i32" => DataType::Int32,
        "int64" | "i64" => DataType::Int64,
        "uint8" | "u8" => DataType::UInt8,
        "uint16" | "u16" => DataType::UInt16,
        "uint32" | "u32" => DataType::UInt32,
        "uint64" | "u64" => DataType::UInt64,
        "float32" | "f32" => DataType::Float32,
        "float64" | "f64" => DataType::Float64,
        "bool" | "boolean" => DataType::Boolean,
        "date" => DataType::Date,
        "datetime" | "dt" => DataType::Datetime(TimeUnit::Microseconds, None),
        "string" | "str" | "utf8" => DataType::String,
        "categorical" | "category" | "cat" => {
            use polars_dtype::categorical::Categories;
            DataType::from_categories(Categories::global())
        }
        _ => return Err(format!("Unknown database dtype '{s}'")),
    };
    Ok(dtype)
}

pub fn dtype_to_string(dt: &DataType) -> Result<String, String> {
    let dtype = match dt {
        DataType::Int8 => "Int8".into(),
        DataType::Int16 => "Int16".into(),
        DataType::Int32 => "Int32".into(),
        DataType::Int64 => "Int64".into(),
        DataType::UInt8 => "UInt8".into(),
        DataType::UInt16 => "UInt16".into(),
        DataType::UInt32 => "UInt32".into(),
        DataType::UInt64 => "UInt64".into(),
        DataType::Float32 => "Float32".into(),
        DataType::Float64 => "Float64".into(),
        DataType::Boolean => "Boolean".into(),
        DataType::String => "String".into(),
        DataType::Categorical(_, _) => "Categorical".into(),
        DataType::Date => "Date".into(),
        DataType::Datetime(_, _) => "DateTime".into(),
        _ => return Err(format!("Unsupported database dtype {dt:?}")),
    };
    Ok(dtype)
}

pub fn apply_operation(df: &mut DataFrame, op: &EditOperation) -> Result<(), String> {
    match op {
        EditOperation::EditCell {
            row,
            col,
            new_value,
            ..
        } => set_cell(df, *row, col, new_value),
        EditOperation::AddRow { index, .. } => {
            let height = df.height();
            let idx = (*index).min(height);

            let null_row: Vec<Column> = df
                .columns()
                .iter()
                .map(|c| {
                    let s = Series::new_null(c.name().clone(), 1);
                    let casted = s.cast(c.dtype()).unwrap_or(s);
                    Column::from(casted)
                })
                .collect();

            let null_df = DataFrame::new(1, null_row)
                .map_err(|e: polars::prelude::PolarsError| e.to_string())?;

            let top = df.slice(0, idx);
            let bottom = df.slice(idx as i64, height - idx);

            *df = top
                .vstack(&null_df)
                .map_err(|e| e.to_string())?
                .vstack(&bottom)
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        EditOperation::DeleteRow { index, .. } => {
            let height = df.height();
            if *index >= height {
                return Err(format!(
                    "Row index {} out of bounds (height={})",
                    index, height
                ));
            }
            let top = df.slice(0, *index);
            let bottom = df.slice((*index + 1) as i64, height - *index - 1);
            *df = top.vstack(&bottom).map_err(|e| e.to_string())?;
            Ok(())
        }
        EditOperation::AddColumn { name, dtype } => {
            let dt = dtype_from_string(dtype)?;
            let s = Series::new_null(PlSmallStr::from(name.as_str()), df.height());
            let casted = s.cast(&dt).map_err(|e| e.to_string())?;
            df.with_column(Column::from(casted))
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        EditOperation::DeleteColumn { name, .. } => {
            df.drop_in_place(&PlSmallStr::from(name.as_str()))
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        EditOperation::RenameColumn { old_name, new_name } => {
            df.rename(
                &PlSmallStr::from(old_name.as_str()),
                PlSmallStr::from(new_name.as_str()),
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        }
        EditOperation::CastColumn { col, new_dtype, .. } => cast_column(df, col, new_dtype, true),
    }
}

pub fn reverse_operation(df: &mut DataFrame, op: &EditOperation) -> Result<(), String> {
    match op {
        EditOperation::EditCell {
            row,
            col,
            old_value,
            ..
        } => set_cell(df, *row, col, old_value),
        EditOperation::AddRow { index, row_id } => {
            let del_op = EditOperation::DeleteRow {
                index: *index,
                row_id: *row_id,
                data: vec![],
            };
            apply_operation(df, &del_op)
        }
        EditOperation::DeleteRow { index, data, .. } => {
            let add_op = EditOperation::AddRow {
                index: *index,
                row_id: None,
            };
            apply_operation(df, &add_op)?;
            for (col_idx, val) in data.iter().enumerate() {
                if col_idx < df.width() {
                    let col_name = df.columns()[col_idx].name().to_string();
                    set_cell(df, *index, &col_name, val)?;
                }
            }
            Ok(())
        }
        EditOperation::AddColumn { name, dtype } => {
            let del_op = EditOperation::DeleteColumn {
                name: name.clone(),
                dtype: dtype.clone(),
                row_ids: vec![],
                row_fingerprints: vec![],
                data: vec![],
            };
            apply_operation(df, &del_op)
        }
        EditOperation::DeleteColumn {
            name, dtype, data, ..
        } => {
            let dtype = dtype_from_string(dtype)?;
            let values: Vec<AnyValue<'static>> = data
                .iter()
                .map(|value| json_to_anyvalue(value, &dtype))
                .collect::<Result<_, _>>()
                .map_err(|error| error.to_string())?;
            let restored = Series::from_any_values(PlSmallStr::from(name.as_str()), &values, false)
                .map_err(|e| e.to_string())?
                .cast(&dtype)
                .map_err(|e| e.to_string())?;
            df.with_column(Column::from(restored))
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        EditOperation::RenameColumn { old_name, new_name } => {
            let rev = EditOperation::RenameColumn {
                old_name: new_name.clone(),
                new_name: old_name.clone(),
            };
            apply_operation(df, &rev)
        }
        EditOperation::CastColumn {
            col,
            old_data,
            old_dtype,
            ..
        } => {
            let col_idx = df
                .get_column_index(col)
                .ok_or_else(|| format!("Column '{}' not found", col))?;
            let dt = dtype_from_string(old_dtype)?;
            let values: Vec<AnyValue<'static>> = old_data
                .iter()
                .map(|v| json_to_anyvalue(v, &dt))
                .collect::<Result<_, _>>()
                .map_err(|error| error.to_string())?;
            let name = df.columns()[col_idx].name().clone();
            let restored = Series::from_any_values(name, &values, false)
                .map_err(|e| e.to_string())?
                .cast(&dt)
                .map_err(|e| e.to_string())?;
            df.replace_column(col_idx, Column::from(restored))
                .map_err(|e| e.to_string())?;
            Ok(())
        }
    }
}

/// Cast a column to a new type. Returns Err if conversion fails (unless force=true).
/// When force=false: uses strict cast, fails if any value cannot be converted.
/// When force=true: uses non-strict cast, invalid values become null.
pub fn cast_column(
    df: &mut DataFrame,
    col_name: &str,
    new_dtype_str: &str,
    force: bool,
) -> Result<(), String> {
    let col_idx = df
        .get_column_index(col_name)
        .ok_or_else(|| format!("Column '{}' not found", col_name))?;
    let series = df.columns()[col_idx].as_materialized_series();
    let target_dtype = dtype_from_string(new_dtype_str)?;

    let casted = if force {
        series
            .cast_with_options(&target_dtype, CastOptions::NonStrict)
            .map_err(|e| e.to_string())?
    } else {
        series
            .strict_cast(&target_dtype)
            .map_err(|e| e.to_string())?
    };

    let new_col = Column::from(casted);
    df.replace_column(col_idx, new_col)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn capture_row_data(df: &DataFrame, row: usize) -> Vec<serde_json::Value> {
    df.columns()
        .iter()
        .map(|c| {
            c.get(row)
                .map(|v| anyvalue_to_json(v))
                .unwrap_or(serde_json::Value::Null)
        })
        .collect()
}

pub fn capture_column_data(df: &DataFrame, col_name: &str) -> Vec<serde_json::Value> {
    df.column(col_name)
        .map(|c| {
            (0..c.len())
                .map(|i| {
                    c.get(i)
                        .map(|v| anyvalue_to_json(v))
                        .unwrap_or(serde_json::Value::Null)
                })
                .collect()
        })
        .unwrap_or_default()
}
