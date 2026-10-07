use serde::{Deserialize, Serialize};

/// External data admitted by the project importer before materialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseImportSource {
    Csv {
        path: String,
        delimiter: char,
        has_header: bool,
        infer_schema_length: Option<usize>,
    },
    Sql {
        engine: DatabaseEngineSql,
        connection_string: String,
        table: String,
    },
    Parquet {
        path: String,
        columns: Option<Vec<String>>,
    },
    Excel {
        path: String,
        sheet: String,
    },
}

/// Project-owned storage identity. External sources belong to DatabaseImportSource.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseEngine {
    Dataset {},
}

/// Connection configuration for an external [`DatabaseImportSource::Sql`] source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseEngineSql {
    Sqlite { auto_create: bool },
    Postgres { ssl: bool },
    Mysql { charset: String },
}
