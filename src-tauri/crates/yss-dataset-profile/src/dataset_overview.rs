use polars::prelude::{BooleanChunked, DataFrame, NamedFrom, PlSmallStr};
use serde::Serialize;

use crate::{ProfileColumnKind, profile_column_kind};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeShape {
    pub n_rows: usize,
    pub n_columns: usize,
    pub estimated_dataframe_memory_bytes: Option<usize>,
    pub duplicated_rows: Option<usize>,
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

pub fn compute_dataset_overview(dataframe: &DataFrame) -> DatasetOverview {
    let n_rows = dataframe.height();
    let n_columns = dataframe.width();
    let duplicated_rows = dataframe
        .is_duplicated()
        .ok()
        .map(|mask| mask.sum().unwrap_or_default() as usize);

    let mut schema_overview = SchemaOverview {
        numeric_cols: 0,
        categorical_cols: 0,
        string_cols: 0,
        datetime_cols: 0,
        bool_cols: 0,
    };
    for (_, data_type) in dataframe.schema().iter() {
        match profile_column_kind(data_type) {
            ProfileColumnKind::Numeric => schema_overview.numeric_cols += 1,
            ProfileColumnKind::Categorical => schema_overview.categorical_cols += 1,
            ProfileColumnKind::String => schema_overview.string_cols += 1,
            ProfileColumnKind::Temporal => schema_overview.datetime_cols += 1,
            ProfileColumnKind::Boolean => schema_overview.bool_cols += 1,
        }
    }

    let total_nulls = dataframe
        .columns()
        .iter()
        .map(|column| column.null_count())
        .sum::<usize>();
    let total_cells = n_rows.saturating_mul(n_columns);
    let null_ratio = if total_cells == 0 {
        0.0
    } else {
        total_nulls as f64 / total_cells as f64
    };
    let cols_with_nulls = dataframe
        .columns()
        .iter()
        .filter(|column| column.null_count() > 0)
        .count();
    let mut rows_with_nulls = BooleanChunked::new(
        PlSmallStr::from_static("dataset_profile_has_null"),
        vec![false; n_rows],
    );
    for column in dataframe.columns() {
        rows_with_nulls = rows_with_nulls | column.is_null();
    }

    DatasetOverview {
        size_shape: SizeShape {
            n_rows,
            n_columns,
            estimated_dataframe_memory_bytes: Some(dataframe.estimated_size()),
            duplicated_rows,
        },
        schema_overview,
        data_completeness: DataCompleteness {
            total_nulls,
            null_ratio,
            cols_with_nulls,
            rows_with_nulls: rows_with_nulls.sum().unwrap_or_default() as usize,
        },
    }
}
