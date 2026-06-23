use super::DatabaseEngineSql;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseEngine {
    /// CSV file
    Csv {
        path: String,
        delimiter: char,
        has_header: bool,
        infer_schema_length: Option<usize>,
    },

    /// SQLite（本地文件），table 为选中的表名
    Sql {
        engine: DatabaseEngineSql,
        connection_string: String,
        table: String,
    },

    /// Parquet file
    Parquet {
        path: String,
        columns: Option<Vec<String>>,
    },

    /// Excel 文件（xlsx/xls），sheet 为选中的 Sheet 名
    Excel { path: String, sheet: String },

    /// 项目内 DuckDB 列存（文件导入后的唯一持久化形态）
    DuckDb {
        /// 相对项目根目录的路径，例如 `database/project.duckdb`
        path: String,
        table: String,
    },

    /// In-memory DataFrame (not serializable, runtime only)
    InMemory { name: String },
}

impl DatabaseEngine {
    pub fn duckdb_table(&self) -> Option<(&str, &str)> {
        match self {
            DatabaseEngine::DuckDb { path, table, .. } => Some((path.as_str(), table.as_str())),
            _ => None,
        }
    }
}
