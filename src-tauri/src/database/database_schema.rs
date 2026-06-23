use crate::graph::node::{ColumnSchema, DataSchema};
use crate::graph::value::DataType;
use polars::prelude::{DataFrame, Schema};

use super::DuckDbColumnMeta;

pub fn duckdb_columns_to_schema(columns: &[DuckDbColumnMeta]) -> DataSchema {
    DataSchema {
        columns: columns
            .iter()
            .map(|col| ColumnSchema {
                name: col.name.clone(),
                data_type: polars_type_string_to_data_type(&col.dtype),
            })
            .collect(),
    }
}

pub fn polars_schema_to_data_schema(schema: &Schema) -> DataSchema {
    DataSchema {
        columns: schema
            .iter_names()
            .filter_map(|name| {
                schema.get(name).map(|dt| ColumnSchema {
                    name: name.to_string(),
                    data_type: polars_dtype_to_data_type(dt),
                })
            })
            .collect(),
    }
}

pub fn dataframe_to_schema(df: &DataFrame) -> DataSchema {
    DataSchema {
        columns: df
            .columns()
            .iter()
            .map(|col| ColumnSchema {
                name: col.name().to_string(),
                data_type: polars_dtype_to_data_type(col.dtype()),
            })
            .collect(),
    }
}

/// 将 Polars 列类型映射为 DataType（供节点执行器等使用）
pub fn polars_dtype_to_data_type(dtype: &polars::prelude::DataType) -> DataType {
    match dtype {
        polars::prelude::DataType::Boolean => DataType::Boolean,
        polars::prelude::DataType::Int32 => DataType::Int32,
        polars::prelude::DataType::Int64 => DataType::Int64,
        polars::prelude::DataType::Float32 => DataType::Float32,
        polars::prelude::DataType::Float64 => DataType::Float64,
        polars::prelude::DataType::String => DataType::String,
        polars::prelude::DataType::Date => DataType::Date,
        polars::prelude::DataType::Datetime(_, _) => DataType::Date,
        polars::prelude::DataType::Time => DataType::Date,
        polars::prelude::DataType::Categorical(_, _) => DataType::Categorical,
        polars::prelude::DataType::Enum(_, _) => DataType::Categorical,
        _ => DataType::Any,
    }
}

/// DataView 使用：返回 Polars 原始类型字符串，不做映射
/// 例如 "Date", "Datetime(Microseconds, None)", "Int64", "Utf8"
pub fn polars_dtype_to_raw_string(dtype: &polars::prelude::DataType) -> String {
    format!("{:?}", dtype)
}

/// 图边界使用：当只有类型字符串时，映射为系统 DataType
/// 用于拖拽列到图、类型推断等场景
pub fn polars_type_string_to_data_type(s: &str) -> DataType {
    let t = s.trim();
    if t.is_empty() {
        return DataType::Any;
    }
    match t {
        "Boolean" => DataType::Boolean,
        "Int32" => DataType::Int32,
        "Int64" => DataType::Int64,
        "Float32" => DataType::Float32,
        "Float64" => DataType::Float64,
        "String" | "Utf8" => DataType::String,
        "Date" => DataType::Date,
        _ if t.starts_with("Datetime(") || t.starts_with("DateTime(") => DataType::Date,
        _ if t.starts_with("Time") => DataType::Date,
        _ if t.starts_with("Categorical(") || t.starts_with("Enum(") => DataType::Categorical,
        _ => DataType::Any,
    }
}
