use serde::{Deserialize, Serialize};

use crate::database::schema_snapshot::DatabaseColumnFact;

fn default_csv_delimiter() -> char {
    ','
}
fn default_true() -> bool {
    true
}

/// 列信息（供项目加载和数据库 schema 同步返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfoDTO {
    pub name: String,
    #[serde(rename = "type")]
    pub dtype: String,
}

pub(crate) fn column_info_from_schema(columns: &[DatabaseColumnFact]) -> Vec<ColumnInfoDTO> {
    columns
        .iter()
        .map(|column| ColumnInfoDTO {
            name: column.name().as_str().to_string(),
            dtype: column.data_type().to_string(),
        })
        .collect()
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
    /// 当前数据库是否物化失败；具体错误仅保留在后端。
    pub load_failed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadDatabaseResultDto {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnInfoDTO>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseMetaResultDto {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnInfoDTO>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseRowsResultDto {
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_ids: Vec<i64>,
}

impl From<&crate::database_contract::DatabaseDecl> for DatabaseDeclDTO {
    fn from(value: &crate::database_contract::DatabaseDecl) -> Self {
        Self {
            id: value.id.as_str().to_string(),
            engine: (&value.engine).into(),
            schema_version: value.schema_version,
            required: value.required,
            name: Some(value.name.to_string()),
            columns: None,
            row_count: None,
            column_count: None,
            load_failed: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseImportSqlEngineDTO {
    Sqlite,
    Postgres,
    Mysql,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseImportSourceDTO {
    Sql {
        engine: DatabaseImportSqlEngineDTO,
        #[serde(rename = "connectionString")]
        connection_string: String,
        table: String,
    },
    Csv {
        path: String,
        #[serde(default = "default_csv_delimiter")]
        delimiter: char,
        #[serde(default = "default_true", rename = "hasHeader")]
        has_header: bool,
        #[serde(default, rename = "inferSchemaLength")]
        infer_schema_length: Option<usize>,
    },
    Parquet {
        path: String,
        #[serde(default)]
        columns: Option<Vec<String>>,
    },
    Excel {
        path: String,
        sheet: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseEngineDTO {
    /// SQLite（本地文件），table 为选中的表名
    Sql {
        engine: DatabaseEngineSqlDTO,
        #[serde(rename = "connectionString")]
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
    Excel { path: String, sheet: String },

    /// 项目内 DuckDB 列存
    DuckDb { path: String, table: String },

    /// In-memory DataFrame (not serializable, runtime only)
    /// Will be ignored or converted during serialization
    InMemory { name: String },
}

impl From<DatabaseImportSourceDTO> for DatabaseEngineDTO {
    fn from(value: DatabaseImportSourceDTO) -> Self {
        match value {
            DatabaseImportSourceDTO::Sql {
                engine,
                connection_string,
                table,
            } => {
                let engine = match engine {
                    DatabaseImportSqlEngineDTO::Sqlite => {
                        DatabaseEngineSqlDTO::Sqlite { auto_create: false }
                    }
                    DatabaseImportSqlEngineDTO::Postgres => {
                        DatabaseEngineSqlDTO::Postgres { ssl: true }
                    }
                    DatabaseImportSqlEngineDTO::Mysql => DatabaseEngineSqlDTO::Mysql {
                        charset: "utf8mb4".into(),
                    },
                };
                Self::Sql {
                    engine,
                    connection_string,
                    table,
                }
            }
            DatabaseImportSourceDTO::Csv {
                path,
                delimiter,
                has_header,
                infer_schema_length,
            } => Self::Csv {
                path,
                delimiter,
                has_header,
                infer_schema_length,
            },
            DatabaseImportSourceDTO::Parquet { path, columns } => Self::Parquet { path, columns },
            DatabaseImportSourceDTO::Excel { path, sheet } => Self::Excel { path, sheet },
        }
    }
}

impl From<&crate::database_contract::DatabaseEngine> for DatabaseEngineDTO {
    fn from(value: &crate::database_contract::DatabaseEngine) -> Self {
        match value {
            crate::database_contract::DatabaseEngine::Sql {
                engine,
                connection_string,
                table,
            } => DatabaseEngineDTO::Sql {
                engine: engine.into(),
                connection_string: connection_string.clone(),
                table: table.clone(),
            },
            crate::database_contract::DatabaseEngine::Csv {
                path,
                delimiter,
                has_header,
                infer_schema_length,
            } => DatabaseEngineDTO::Csv {
                path: path.clone(),
                delimiter: *delimiter,
                has_header: *has_header,
                infer_schema_length: *infer_schema_length,
            },
            crate::database_contract::DatabaseEngine::Parquet { path, columns } => {
                DatabaseEngineDTO::Parquet {
                    path: path.clone(),
                    columns: columns.clone(),
                }
            }
            crate::database_contract::DatabaseEngine::Excel { path, sheet } => {
                DatabaseEngineDTO::Excel {
                    path: path.clone(),
                    sheet: sheet.clone(),
                }
            }
            crate::database_contract::DatabaseEngine::DuckDb { path, table } => {
                DatabaseEngineDTO::DuckDb {
                    path: path.clone(),
                    table: table.clone(),
                }
            }
            crate::database_contract::DatabaseEngine::InMemory { name } => {
                DatabaseEngineDTO::InMemory { name: name.clone() }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseEngineSqlDTO {
    Sqlite {
        #[serde(default, rename = "autoCreate")]
        auto_create: bool,
    },
    Postgres {
        ssl: bool,
    },
    Mysql {
        charset: String,
    },
}

impl From<&crate::database_contract::DatabaseEngineSql> for DatabaseEngineSqlDTO {
    fn from(value: &crate::database_contract::DatabaseEngineSql) -> Self {
        match value {
            crate::database_contract::DatabaseEngineSql::Sqlite { auto_create } => {
                DatabaseEngineSqlDTO::Sqlite {
                    auto_create: *auto_create,
                }
            }
            crate::database_contract::DatabaseEngineSql::Postgres { ssl } => {
                DatabaseEngineSqlDTO::Postgres { ssl: *ssl }
            }
            crate::database_contract::DatabaseEngineSql::Mysql { charset } => {
                DatabaseEngineSqlDTO::Mysql {
                    charset: charset.clone(),
                }
            }
        }
    }
}

impl TryFrom<DatabaseEngineSqlDTO> for crate::database_contract::DatabaseEngineSql {
    type Error = String;

    fn try_from(dto: DatabaseEngineSqlDTO) -> Result<Self, Self::Error> {
        match dto {
            DatabaseEngineSqlDTO::Sqlite { auto_create } => {
                Ok(crate::database_contract::DatabaseEngineSql::Sqlite { auto_create })
            }
            DatabaseEngineSqlDTO::Postgres { ssl } => {
                Ok(crate::database_contract::DatabaseEngineSql::Postgres { ssl })
            }
            DatabaseEngineSqlDTO::Mysql { charset } => {
                Ok(crate::database_contract::DatabaseEngineSql::Mysql { charset })
            }
        }
    }
}

impl TryFrom<DatabaseEngineDTO> for crate::database_contract::DatabaseEngine {
    type Error = String;

    fn try_from(dto: DatabaseEngineDTO) -> Result<Self, Self::Error> {
        match dto {
            DatabaseEngineDTO::Csv {
                path,
                delimiter,
                has_header,
                infer_schema_length,
            } => Ok(crate::database_contract::DatabaseEngine::Csv {
                path,
                delimiter,
                has_header,
                infer_schema_length,
            }),
            DatabaseEngineDTO::Parquet { path, columns } => {
                Ok(crate::database_contract::DatabaseEngine::Parquet { path, columns })
            }
            DatabaseEngineDTO::Excel { path, sheet } => {
                Ok(crate::database_contract::DatabaseEngine::Excel { path, sheet })
            }
            DatabaseEngineDTO::DuckDb { path, table } => {
                Ok(crate::database_contract::DatabaseEngine::DuckDb { path, table })
            }
            DatabaseEngineDTO::Sql {
                engine,
                connection_string,
                table,
            } => {
                let engine = crate::database_contract::DatabaseEngineSql::try_from(engine)?;
                Ok(crate::database_contract::DatabaseEngine::Sql {
                    engine,
                    connection_string,
                    table,
                })
            }
            DatabaseEngineDTO::InMemory { name } => {
                Ok(crate::database_contract::DatabaseEngine::InMemory { name })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn database_import_source_wire_contains_only_effective_inputs() {
        let source = serde_json::from_value::<DatabaseImportSourceDTO>(json!({
            "sql": {
                "engine": "sqlite",
                "connectionString": "C:/data/source.sqlite",
                "table": "sales"
            }
        }))
        .unwrap();
        assert!(matches!(
            source,
            DatabaseImportSourceDTO::Sql {
                engine: DatabaseImportSqlEngineDTO::Sqlite,
                ..
            }
        ));

        for invalid in [
            json!({ "duckDb": { "path": "database/project.duckdb", "table": "sales" } }),
            json!({ "inMemory": { "name": "sales" } }),
            json!({
                "sql": {
                    "engine": { "sqlite": { "autoCreate": false } },
                    "connectionString": "C:/data/source.sqlite",
                    "table": "sales"
                }
            }),
        ] {
            assert!(serde_json::from_value::<DatabaseImportSourceDTO>(invalid).is_err());
        }
    }
}
