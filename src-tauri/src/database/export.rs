use std::path::Path;

use polars::prelude::DataFrame;

use crate::tabular::dataframe_io::{write_csv_dataframe, write_parquet_dataframe};

pub fn export_dataframe(df: &mut DataFrame, path: &str, format: &str) -> Result<(), String> {
    let path = Path::new(path);
    match format.to_lowercase().as_str() {
        "csv" => write_csv_dataframe(path, df),
        "parquet" => write_parquet_dataframe(path, df),
        _ => Err(format!("Unsupported export format: {format}")),
    }
}
