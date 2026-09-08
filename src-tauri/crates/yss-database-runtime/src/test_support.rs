//! Owned temporary DuckDB tables for runtime and consumer contract tests.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use polars::prelude::DataFrame;
use yss_database_contract::{DatabaseDecl, DatabaseEngine, DatabaseId};
use yss_database_edit::EditHistory;

use crate::{DatabaseInstance, DatabaseState};

static NEXT_DATABASE: AtomicU64 = AtomicU64::new(0);

pub struct DuckDbFixture {
    path: PathBuf,
    pub instance: DatabaseInstance,
}

impl DuckDbFixture {
    pub fn new(id: &str, mut dataframe: DataFrame) -> Self {
        let sequence = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "yssbi-database-{}-{timestamp}-{sequence}.duckdb",
            std::process::id(),
        ));
        let meta = yss_duckdb::ingest_dataframe_to_duckdb(&mut dataframe, &path, id)
            .expect("create test DuckDB table");
        let duckdb_path = path.to_string_lossy().into_owned();
        Self {
            path,
            instance: DatabaseInstance {
                decl: DatabaseDecl {
                    id: DatabaseId::from_existing(id.into()),
                    engine: DatabaseEngine::DuckDb {
                        path: duckdb_path.clone(),
                        table: id.into(),
                    },
                    schema_version: 1,
                    required: false,
                    name: id.into(),
                },
                state: DatabaseState::DuckDb {
                    duckdb_path,
                    table: id.into(),
                    row_count: meta.row_count,
                    columns: meta.columns,
                    history: EditHistory::new(),
                },
            },
        }
    }
}

impl Drop for DuckDbFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
