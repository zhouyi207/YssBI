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
    Excel {
        path: String,
        sheet: String,
    },

    /// In-memory DataFrame (not serializable, runtime only)
    /// Will be ignored or converted during serialization
    InMemory { name: String },
}

impl DatabaseEngine {
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

            DatabaseEngine::Sql { engine, connection_string, table } => {
                let df = super::sql_reader::read_table_to_dataframe(engine, connection_string, table)
                    .map_err(|e| PolarsError::ComputeError(e.into()))?;
                Ok(df.lazy())
            }

            DatabaseEngine::Excel { path, sheet } => {
                let df = super::excel_reader::read_sheet_to_dataframe(path, sheet)
                    .map_err(|e| PolarsError::ComputeError(e.into()))?;
                Ok(df.lazy())
            }

            DatabaseEngine::InMemory { .. } => Err(PolarsError::ComputeError(
                "InMemory engine has no lazy source".into(),
            )),
        }
    }
}
