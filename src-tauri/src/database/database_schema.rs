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
    use polars::prelude::DataType as PDataType;
    match dtype {
        PDataType::Boolean => DataType::Boolean,
        // 所有整数宽度收敛到 Int64（运行时规范类型）
        PDataType::Int8
        | PDataType::Int16
        | PDataType::Int32
        | PDataType::Int64
        | PDataType::UInt8
        | PDataType::UInt16
        | PDataType::UInt32
        | PDataType::UInt64 => DataType::Int64,
        // 所有浮点宽度 + Decimal 收敛到 Float64
        PDataType::Float32 | PDataType::Float64 | PDataType::Decimal(_, _) => DataType::Float64,
        PDataType::String => DataType::String,
        PDataType::Date => DataType::Date,
        PDataType::Datetime(_, _) => DataType::Datetime,
        PDataType::Time => DataType::Time,
        PDataType::Categorical(_, _) | PDataType::Enum(_, _) => DataType::Categorical,
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
        "Int8" | "Int16" | "Int32" | "Int64" | "UInt8" | "UInt16" | "UInt32" | "UInt64" => {
            DataType::Int64
        }
        "Float32" | "Float64" => DataType::Float64,
        "String" | "Utf8" => DataType::String,
        "Date" => DataType::Date,
        "Time" => DataType::Time,
        _ if t.starts_with("Datetime(") || t.starts_with("DateTime(") => DataType::Datetime,
        _ if t.starts_with("Time") => DataType::Time,
        _ if t.starts_with("Decimal(") => DataType::Float64,
        _ if t.starts_with("Categorical(") || t.starts_with("Enum(") => DataType::Categorical,
        _ => DataType::Any,
    }
}
