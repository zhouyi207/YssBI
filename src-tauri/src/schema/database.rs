use serde::{Deserialize, Serialize};

fn default_csv_delimiter() -> char {
    ','
}
fn default_true() -> bool {
    true
}

/// 列信息（供 get_project_data 返回 schema 用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfoDTO {
    pub name: String,
    #[serde(rename = "type")]
    pub dtype: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDeclDTO {
    pub id: String,
    pub engine: DatabaseEngineDTO,
    pub schema_version: u32,
    pub required: bool,
    /// 从 project_store 补充的 schema 信息（加载项目后可用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<ColumnInfoDTO>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_count: Option<usize>,
}

impl From<&crate::database::DatabaseDecl> for DatabaseDeclDTO {
    fn from(value: &crate::database::DatabaseDecl) -> Self {
        Self {
            id: value.id.clone(),
            engine: (&value.engine).into(),
            schema_version: value.schema_version,
            required: value.required,
            name: None,
            columns: None,
            row_count: None,
            column_count: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseEngineDTO {
    /// SQLite（本地文件），table 为选中的表名
    Sql {
        engine: DatabaseEngineSqlDTO,
        #[serde(rename = "connectionString", alias = "connection_string")]
        connection_string: String,
        table: String,
    },

    /// CSV file
    Csv {
        path: String,
        #[serde(default = "default_csv_delimiter")]
        delimiter: char,
        #[serde(default = "default_true", rename = "hasHeader")]
        has_header: bool,
        #[serde(default, rename = "inferSchemaLength")]
        infer_schema_length: Option<usize>,
    },

    /// Parquet file
    Parquet {
        path: String,
        columns: Option<Vec<String>>,
    },

    /// Excel 文件（xlsx/xls），sheet 为选中的 Sheet 名
    Excel {
        path: String,
        sheet: String,
    },

    /// In-memory DataFrame (not serializable, runtime only)
    /// Will be ignored or converted during serialization
    InMemory { name: String },
}

impl From<&crate::database::DatabaseEngine> for DatabaseEngineDTO {
    fn from(value: &crate::database::DatabaseEngine) -> Self {
        match value {
            crate::database::DatabaseEngine::Sql { engine, connection_string, table } => {
                DatabaseEngineDTO::Sql {
                    engine: engine.into(),
                    connection_string: connection_string.clone(),
                    table: table.clone(),
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
            crate::database::DatabaseEngine::Excel { path, sheet } => {
                DatabaseEngineDTO::Excel {
                    path: path.clone(),
                    sheet: sheet.clone(),
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
#[serde(rename_all = "camelCase")]
pub enum DatabaseEngineSqlDTO {
    Sqlite {
        #[serde(default, rename = "autoCreate", alias = "auto_create")]
        auto_create: bool,
    },
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

impl TryFrom<DatabaseEngineSqlDTO> for crate::database::DatabaseEngineSql {
    type Error = String;

    fn try_from(dto: DatabaseEngineSqlDTO) -> Result<Self, Self::Error> {
        match dto {
            DatabaseEngineSqlDTO::Sqlite { auto_create } => {
                Ok(crate::database::DatabaseEngineSql::Sqlite { auto_create })
            }
            DatabaseEngineSqlDTO::Postgres { ssl } => {
                Ok(crate::database::DatabaseEngineSql::Postgres { ssl })
            }
            DatabaseEngineSqlDTO::Mysql { charset } => {
                Ok(crate::database::DatabaseEngineSql::Mysql { charset })
            }
        }
    }
}

impl TryFrom<DatabaseEngineDTO> for crate::database::DatabaseEngine {
    type Error = String;

    fn try_from(dto: DatabaseEngineDTO) -> Result<Self, Self::Error> {
        match dto {
            DatabaseEngineDTO::Csv {
                path,
                delimiter,
                has_header,
                infer_schema_length,
            } => Ok(crate::database::DatabaseEngine::Csv {
                path,
                delimiter,
                has_header,
                infer_schema_length,
            }),
            DatabaseEngineDTO::Parquet { path, columns } => {
                Ok(crate::database::DatabaseEngine::Parquet { path, columns })
            }
            DatabaseEngineDTO::Excel { path, sheet } => {
                Ok(crate::database::DatabaseEngine::Excel { path, sheet })
            }
            DatabaseEngineDTO::Sql {
                engine,
                connection_string,
                table,
            } => {
                let engine = crate::database::DatabaseEngineSql::try_from(engine)?;
                Ok(crate::database::DatabaseEngine::Sql {
                    engine,
                    connection_string,
                    table,
                })
            }
            DatabaseEngineDTO::InMemory { name } => {
                Ok(crate::database::DatabaseEngine::InMemory { name })
            }
        }
    }
}
