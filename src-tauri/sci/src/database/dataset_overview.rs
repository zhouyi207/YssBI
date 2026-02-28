use polars::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeShape {
    pub n_rows: usize,
    pub n_columns: usize,
    pub memory_size: usize,
    pub duplicated_rows: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaOverview {
    pub numeric_cols: usize,
    pub categorical_cols: usize,
    pub string_cols: usize,
    pub datetime_cols: usize,
    pub bool_cols: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCompleteness {
    pub total_nulls: usize,
    pub null_ratio: f64,
    pub cols_with_nulls: usize,
    pub rows_with_nulls: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetOverview {
    pub size_shape: SizeShape,
    pub schema_overview: SchemaOverview,
    pub data_completeness: DataCompleteness,
}

fn is_numeric(dt: &DataType) -> bool {
    matches!(
        dt,
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float32
            | DataType::Float64
    )
}

fn is_categorical(dt: &DataType) -> bool {
    matches!(dt, DataType::Categorical(_, _) | DataType::Enum(_, _))
}

fn is_string(dt: &DataType) -> bool {
    matches!(dt, DataType::String)
}

fn is_datetime(dt: &DataType) -> bool {
    matches!(dt, DataType::Date | DataType::Time | DataType::Datetime(_, _) | DataType::Duration(_))
}

fn is_bool(dt: &DataType) -> bool {
    matches!(dt, DataType::Boolean)
}

pub fn compute_dataset_overview(df: &DataFrame) -> DatasetOverview {
    let n_rows = df.height();
    let n_columns = df.width();

    let memory_size = df.estimated_size();

    let duplicated_rows = df
        .is_duplicated()
        .map(|mask| mask.sum().unwrap_or(0) as usize)
        .unwrap_or(0);

    let schema = df.schema();
    let mut numeric_cols = 0usize;
    let mut categorical_cols = 0usize;
    let mut string_cols = 0usize;
    let mut datetime_cols = 0usize;
    let mut bool_cols = 0usize;

    for (_, dt) in schema.iter() {
        if is_numeric(dt) {
            numeric_cols += 1;
        } else if is_categorical(dt) {
            categorical_cols += 1;
        } else if is_string(dt) {
            string_cols += 1;
        } else if is_datetime(dt) {
            datetime_cols += 1;
        } else if is_bool(dt) {
            bool_cols += 1;
        }
    }

    let total_nulls: usize = df.get_columns().iter().map(|c| c.null_count()).sum();
    let total_cells = n_rows * n_columns;
    let null_ratio = if total_cells > 0 {
        total_nulls as f64 / total_cells as f64
    } else {
        0.0
    };
    let cols_with_nulls = df.get_columns().iter().filter(|c| c.null_count() > 0).count();

    let rows_with_nulls = {
        let mut has_null = BooleanChunked::new(PlSmallStr::from_static("_"), vec![false; n_rows]);
        for col in df.get_columns() {
            let is_null = col.is_null();
            has_null = has_null | is_null;
        }
        has_null.sum().unwrap_or(0) as usize
    };

    DatasetOverview {
        size_shape: SizeShape {
            n_rows,
            n_columns,
            memory_size,
            duplicated_rows,
        },
        schema_overview: SchemaOverview {
            numeric_cols,
            categorical_cols,
            string_cols,
            datetime_cols,
            bool_cols,
        },
        data_completeness: DataCompleteness {
            total_nulls,
            null_ratio,
            cols_with_nulls,
            rows_with_nulls,
        },
    }
}
