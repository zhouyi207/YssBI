use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseDeclDTO {
    pub id: String,
    pub engine: DatabaseEngineDTO,
    pub schema_version: u32,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseEngineDTO {
    /// SQLite（本地文件）
    Sql {
        engine: DatabaseEngineSqlDTO,
        connection_string: String,
    },

    /// CSV file
    Csv {
        path: String,
        delimiter: char,
        has_header: bool,
        infer_schema_length: Option<usize>,
    },

    /// Parquet file
    Parquet {
        path: String,
        columns: Option<Vec<String>>,
    },

    /// In-memory DataFrame (not serializable, runtime only)
    /// Will be ignored or converted during serialization
    InMemory { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DatabaseEngineSqlDTO {
    Sqlite { auto_create: bool },
    Postgres { ssl: bool },
    Mysql { charset: String },
}
