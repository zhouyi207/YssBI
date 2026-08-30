use serde::{Deserialize, Serialize};

/// Supported database sources persisted in a project declaration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseEngine {
    /// CSV file.
    Csv {
        path: String,
        delimiter: char,
        has_header: bool,
        infer_schema_length: Option<usize>,
    },

    /// SQL database table.
    Sql {
        engine: DatabaseEngineSql,
        connection_string: String,
        table: String,
    },

    /// Parquet file.
    Parquet {
        path: String,
        columns: Option<Vec<String>>,
    },

    /// Excel workbook sheet.
    Excel { path: String, sheet: String },

    /// Project-local DuckDB table.
    DuckDb { path: String, table: String },

    /// Runtime-only in-memory DataFrame identity.
    InMemory { name: String },
}

impl DatabaseEngine {
    #[must_use]
    pub fn duckdb_table(&self) -> Option<(&str, &str)> {
        match self {
            Self::DuckDb { path, table } => Some((path.as_str(), table.as_str())),
            _ => None,
        }
    }
}

/// SQL engine configuration persisted inside a [`DatabaseEngine::Sql`] value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseEngineSql {
    Sqlite { auto_create: bool },
    Postgres { ssl: bool },
    Mysql { charset: String },
}
