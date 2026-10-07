use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DatabaseExportFormat {
    Csv,
    Parquet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unsupported database export format")]
pub struct DatabaseExportFormatParseError;

impl FromStr for DatabaseExportFormat {
    type Err = DatabaseExportFormatParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "csv" => Ok(Self::Csv),
            "parquet" => Ok(Self::Parquet),
            _ => Err(DatabaseExportFormatParseError),
        }
    }
}
