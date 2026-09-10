use serde::{Deserialize, Serialize};

use yss_database_schema::DatabaseColumnFact;

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
            dtype: column.display_type().to_owned(),
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

impl From<&yss_database_contract::DatabaseDecl> for DatabaseDeclDTO {
    fn from(value: &yss_database_contract::DatabaseDecl) -> Self {
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
    Dataset {},
}

impl From<DatabaseImportSourceDTO> for yss_database_contract::DatabaseImportSource {
    fn from(value: DatabaseImportSourceDTO) -> Self {
        match value {
            DatabaseImportSourceDTO::Sql {
                engine,
                connection_string,
                table,
            } => {
                let engine = match engine {
                    DatabaseImportSqlEngineDTO::Sqlite => {
                        yss_database_contract::DatabaseEngineSql::Sqlite { auto_create: false }
                    }
                    DatabaseImportSqlEngineDTO::Postgres => {
                        yss_database_contract::DatabaseEngineSql::Postgres { ssl: true }
                    }
                    DatabaseImportSqlEngineDTO::Mysql => {
                        yss_database_contract::DatabaseEngineSql::Mysql {
                            charset: "utf8mb4".into(),
                        }
                    }
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

impl From<&yss_database_contract::DatabaseEngine> for DatabaseEngineDTO {
    fn from(value: &yss_database_contract::DatabaseEngine) -> Self {
        match value {
            yss_database_contract::DatabaseEngine::Dataset {} => Self::Dataset {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use yss_database_contract::DatabaseId;
    use arrow::datatypes::{DataType, Field, Schema};

    #[test]
    fn database_declaration_wire_exposes_only_machine_load_state() {
        let wire = serde_json::to_value(DatabaseDeclDTO {
            id: "sales".into(),
            engine: DatabaseEngineDTO::Dataset {},
            schema_version: 1,
            required: false,
            name: Some("Sales".into()),
            columns: None,
            row_count: None,
            column_count: None,
            load_failed: true,
        })
        .unwrap();

        assert_eq!(
            wire,
            json!({
                "id": "sales",
                "engine": { "dataset": {} },
                "schemaVersion": 1,
                "required": false,
                "name": "Sales",
                "loadFailed": true,
            })
        );
    }

    #[test]
    fn database_schema_facts_map_to_column_info_dto_wire() {
        let database = DatabaseId::from_existing("database-id".into());
        let schema = Schema::new(vec![Field::new("value", DataType::UInt64, true), Field::new("label", DataType::Utf8, true)]);
        let fact = yss_tabular_arrow::database_schema_fact(&database, &schema).unwrap();

        let wire = serde_json::to_value(column_info_from_schema(fact.columns()))
            .expect("column info DTOs should serialize");

        assert_eq!(
            wire,
            json!([
                { "name": "value", "type": "UInt64" },
                { "name": "label", "type": "String" },
            ])
        );
    }

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
        let source: yss_database_contract::DatabaseImportSource = source.into();
        assert_eq!(
            source,
            yss_database_contract::DatabaseImportSource::Sql {
                engine: yss_database_contract::DatabaseEngineSql::Sqlite { auto_create: false },
                connection_string: "C:/data/source.sqlite".into(),
                table: "sales".into(),
            }
        );

        let csv = serde_json::from_value::<DatabaseImportSourceDTO>(json!({
            "csv": { "path": "data.csv" }
        }))
        .unwrap();
        let csv: yss_database_contract::DatabaseImportSource = csv.into();
        assert_eq!(
            csv,
            yss_database_contract::DatabaseImportSource::Csv {
                path: "data.csv".into(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: None,
            }
        );

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
