use serde::Serialize;

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
