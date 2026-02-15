use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseDeclDTO {
    pub id: String,
    pub engine: DatabaseEngineDTO,
    pub schema_version: u32,
    pub required: bool,
}

impl From<&crate::database::DatabaseDecl> for DatabaseDeclDTO {
    fn from(value: &crate::database::DatabaseDecl) -> Self {
        Self {
            id: value.id.clone(),
            engine: (&value.engine).into(),
            schema_version: value.schema_version,
            required: value.required,
        }
    }
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

impl From<&crate::database::DatabaseEngine> for DatabaseEngineDTO {
    fn from(value: &crate::database::DatabaseEngine) -> Self {
        match value {
            crate::database::DatabaseEngine::Sql { engine, connection_string } => {
                DatabaseEngineDTO::Sql {
                    engine: engine.into(),
                    connection_string: connection_string.clone(),
                }
            }
            crate::database::DatabaseEngine::Csv { path, delimiter, has_header, infer_schema_length } => {
                DatabaseEngineDTO::Csv {
                    path: path.clone(),
                    delimiter: *delimiter,
                    has_header: *has_header,
                    infer_schema_length: *infer_schema_length,
                }
            }
            crate::database::DatabaseEngine::Parquet { path, columns } => {
                DatabaseEngineDTO::Parquet {
                    path: path.clone(),
                    columns: columns.clone(),
                }
            }
            crate::database::DatabaseEngine::InMemory { name } => {
                DatabaseEngineDTO::InMemory {
                    name: name.clone(),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DatabaseEngineSqlDTO {
    Sqlite { auto_create: bool },
    Postgres { ssl: bool },
    Mysql { charset: String },
}

impl From<&crate::database::DatabaseEngineSql> for DatabaseEngineSqlDTO {
    fn from(value: &crate::database::DatabaseEngineSql) -> Self {
        match value {
            crate::database::DatabaseEngineSql::Sqlite { auto_create } => {
                DatabaseEngineSqlDTO::Sqlite { auto_create: *auto_create }
            }
            crate::database::DatabaseEngineSql::Postgres { ssl } => {
                DatabaseEngineSqlDTO::Postgres { ssl: *ssl }
            }
            crate::database::DatabaseEngineSql::Mysql { charset } => {
                DatabaseEngineSqlDTO::Mysql { charset: charset.clone() }
            }
        }
    }
}
