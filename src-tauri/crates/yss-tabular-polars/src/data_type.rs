//! Canonical database dtype names at the Polars boundary.

use polars::prelude::{DataType, TimeUnit};

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
