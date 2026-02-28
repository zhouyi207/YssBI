use polars::prelude::*;
use std::fs::File;
use std::path::Path;

pub fn export_dataframe(df: &mut DataFrame, path: &str, format: &str) -> Result<(), String> {
    let p = Path::new(path);

    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    match format.to_lowercase().as_str() {
        "csv" => {
            let file = File::create(p).map_err(|e| format!("Failed to create file: {}", e))?;
            CsvWriter::new(file)
                .finish(df)
                .map_err(|e| format!("Failed to write CSV: {}", e))
        }
        "parquet" => {
            let file = File::create(p).map_err(|e| format!("Failed to create file: {}", e))?;
            ParquetWriter::new(file)
                .finish(df)
                .map(|_| ())
                .map_err(|e| format!("Failed to write Parquet: {}", e))
        }
        _ => Err(format!("Unsupported export format: {}", format)),
    }
}
