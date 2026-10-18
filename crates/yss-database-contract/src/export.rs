#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DatabaseExportFormat {
    Csv,
    Parquet,
}

impl DatabaseExportFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Parquet => "parquet",
        }
    }
}
