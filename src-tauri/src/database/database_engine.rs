use polars::error::{PolarsError, PolarsResult};
use polars::prelude::LazyFileListReader;
use polars::prelude::{col, IntoLazy, LazyCsvReader, LazyFrame, PlRefPath};

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
        /// 相对项目根目录的路径，例如 `database/db-xxx.duckdb`
        path: String,
        table: String,
    },

    /// In-memory DataFrame (not serializable, runtime only)
    /// Will be ignored or converted during serialization
    InMemory { name: String },
}

impl DatabaseEngine {
    /// 是否拥有「真正」的惰性读取实现：即 `build_lazy()` 仅做轻量的元数据
    /// 解析（如 CSV header / Parquet footer），不会同步把整个数据集拉进内存。
    ///
    /// 当前 polars 对 SQL / Excel 不提供 lazy adapter，我们的 `build_lazy()`
    /// 实际是同步 `read_*_to_dataframe` 后再 `df.lazy()`，所以它们返回 false。
    /// 不属于真·lazy 的引擎在 `ProjectState::set_data` 中会同步物化为
    /// `DatabaseState::Loaded` 或 `Failed`（不再使用后台 Pending 物化）。
    pub fn is_lazy_friendly(&self) -> bool {
        match self {
            DatabaseEngine::Parquet { .. } | DatabaseEngine::InMemory { .. } => true,
            DatabaseEngine::Csv { .. }
            | DatabaseEngine::DuckDb { .. }
            | DatabaseEngine::Sql { .. }
            | DatabaseEngine::Excel { .. } => false,
        }
    }

    pub fn duckdb_table(&self) -> Option<(&str, &str)> {
        match self {
            DatabaseEngine::DuckDb { path, table, .. } => Some((path.as_str(), table.as_str())),
            _ => None,
        }
    }

    pub fn build_lazy(&self) -> PolarsResult<LazyFrame> {
        match self {
            DatabaseEngine::Csv {
                path,
                delimiter,
                has_header,
                infer_schema_length,
            } => {
                let pl_path = PlRefPath::new(path.as_str());

                LazyCsvReader::new(pl_path)
                    .with_separator(*delimiter as u8)
                    .with_has_header(*has_header)
                    .with_infer_schema_length(*infer_schema_length)
                    .with_try_parse_dates(true)
                    .finish()
            }

            DatabaseEngine::Parquet { path, columns } => {
                let pl_path = PlRefPath::new(path.as_str());

                let mut reader = LazyFrame::scan_parquet(pl_path, Default::default())?;
                if let Some(cols) = columns {
                    reader = reader.select(cols.iter().map(|c| col(c)).collect::<Vec<_>>());
                }
                Ok(reader)
            }

            DatabaseEngine::Sql {
                engine,
                connection_string,
                table,
            } => {
                let df =
                    super::sql_reader::read_table_to_dataframe(engine, connection_string, table)
                        .map_err(|e| PolarsError::ComputeError(e.into()))?;
                Ok(df.lazy())
            }

            DatabaseEngine::Excel { path, sheet } => {
                let df = super::excel_reader::read_sheet_to_dataframe(path, sheet)
                    .map_err(|e| PolarsError::ComputeError(e.into()))?;
                Ok(df.lazy())
            }

            DatabaseEngine::DuckDb { .. } => Err(PolarsError::ComputeError(
                "DuckDb engine has no lazy source; use duckdb_reader".into(),
            )),

            DatabaseEngine::InMemory { .. } => Err(PolarsError::ComputeError(
                "InMemory engine has no lazy source".into(),
            )),
        }
    }
}
