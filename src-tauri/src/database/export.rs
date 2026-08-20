use std::path::Path;

use duckdb::Connection;
use polars::prelude::DataFrame;

use crate::tabular::dataframe_io::{write_csv_dataframe, write_parquet_dataframe};

use super::{quote_duckdb_identifier, quote_duckdb_string_literal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseExportFormat {
    Csv,
    Parquet,
}

impl DatabaseExportFormat {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "parquet" => Some(Self::Parquet),
            _ => None,
        }
    }
}

pub fn export_dataframe(
    dataframe: &mut DataFrame,
    path: &Path,
    format: DatabaseExportFormat,
) -> Result<(), String> {
    match format {
        DatabaseExportFormat::Csv => write_csv_dataframe(path, dataframe),
        DatabaseExportFormat::Parquet => write_parquet_dataframe(path, dataframe),
    }
}

pub fn export_duckdb_table(
    duckdb_path: &Path,
    table: &str,
    path: &Path,
    format: DatabaseExportFormat,
) -> Result<(), String> {
    let connection = Connection::open(duckdb_path).map_err(|error| error.to_string())?;
    let table = quote_duckdb_identifier(table);
    let destination = quote_duckdb_string_literal(&path.to_string_lossy().replace('\\', "/"));
    let options = match format {
        DatabaseExportFormat::Csv => "FORMAT CSV, HEADER true",
        DatabaseExportFormat::Parquet => "FORMAT PARQUET",
    };
    connection
        .execute_batch(&format!(
            "COPY (SELECT * FROM {table}) TO {destination} ({options});"
        ))
        .map_err(|error| error.to_string())
}
