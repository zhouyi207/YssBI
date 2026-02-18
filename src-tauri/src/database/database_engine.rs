use polars::error::{PolarsError, PolarsResult};
use polars::prelude::LazyFileListReader;
use polars::prelude::{col, LazyCsvReader, LazyFrame, PlPath};
use std::path::PathBuf;

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

    /// SQLite（本地文件）
    Sql {
        engine: DatabaseEngineSql,
        connection_string: String,
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

impl DatabaseEngine {
    pub fn build_lazy(&self) -> PolarsResult<LazyFrame> {
        match self {
            DatabaseEngine::Csv {
                path,
                delimiter,
                has_header,
                infer_schema_length,
            } => {
                let path = PathBuf::from(path);
                let pl_path = PlPath::Local(path.into());

                LazyCsvReader::new(pl_path)
                    .with_separator(*delimiter as u8)
                    .with_has_header(*has_header)
                    .with_infer_schema_length(*infer_schema_length)
                    .finish()
            }

            DatabaseEngine::Parquet { path, columns } => {
                let path = PathBuf::from(path);
                let pl_path = PlPath::Local(path.into());

                let mut reader = LazyFrame::scan_parquet(pl_path, Default::default())?;
                if let Some(cols) = columns {
                    reader = reader.select(cols.iter().map(|c| col(c)).collect::<Vec<_>>());
                }
                Ok(reader)
            }

            DatabaseEngine::Sql { .. } => {
                // SQL → LazyFrame（可能是 Arrow / DataFusion）
                todo!()
            }

            DatabaseEngine::InMemory { .. } => Err(PolarsError::ComputeError(
                "InMemory engine has no lazy source".into(),
            )),
        }
    }
}
