use crate::graph::node::{ColumnSchema, DataSchema};
use crate::graph::value::DataType;
use polars::prelude::DataFrame;

pub fn dataframe_to_schema(df: &DataFrame) -> DataSchema {
    DataSchema {
        columns: df
            .get_columns()
            .iter()
            .map(|col| ColumnSchema {
                name: col.name().to_string(),
                data_type: polars_dtype_to_data_type(col.dtype()),
            })
            .collect(),
    }
}

fn polars_dtype_to_data_type(dtype: &polars::prelude::DataType) -> DataType {
    match dtype {
        polars::prelude::DataType::Boolean => DataType::Boolean,
        polars::prelude::DataType::Int32 => DataType::Int32,
        polars::prelude::DataType::Int64 => DataType::Int64,
        polars::prelude::DataType::Float32 => DataType::Float32,
        polars::prelude::DataType::Float64 => DataType::Float64,
        polars::prelude::DataType::String => DataType::String,
        _ => DataType::Any,
    }
}
