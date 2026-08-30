use std::path::Path;

use duckdb::Connection;
use yss_database_contract::DatabaseExportFormat;

use crate::{quote_duckdb_identifier, quote_duckdb_string_literal};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DuckDbExportPhase {
    Open,
    Copy,
}

#[derive(Debug, thiserror::Error)]
#[error("DuckDB table export failed")]
pub struct DuckDbExportError {
    phase: DuckDbExportPhase,
    #[source]
    source: duckdb::Error,
}

impl DuckDbExportError {
    pub fn phase(&self) -> DuckDbExportPhase {
        self.phase
    }
}

pub fn export_duckdb_table(
    duckdb_path: &Path,
    table: &str,
    destination: &Path,
    format: DatabaseExportFormat,
) -> Result<(), DuckDbExportError> {
    let connection = Connection::open(duckdb_path).map_err(|source| DuckDbExportError {
        phase: DuckDbExportPhase::Open,
        source,
    })?;
    let table = quote_duckdb_identifier(table);
    let destination =
        quote_duckdb_string_literal(&destination.to_string_lossy().replace('\\', "/"));
    let options = match format {
        DatabaseExportFormat::Csv => "FORMAT CSV, HEADER true",
        DatabaseExportFormat::Parquet => "FORMAT PARQUET",
    };
    connection
        .execute_batch(&format!(
            "COPY (SELECT * FROM {table}) TO {destination} ({options});"
        ))
        .map_err(|source| DuckDbExportError {
            phase: DuckDbExportPhase::Copy,
            source,
        })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use duckdb::Connection;
    use yss_database_contract::DatabaseExportFormat;

    use super::{DuckDbExportPhase, export_duckdb_table};

    struct TestDirectory(PathBuf);

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    impl TestDirectory {
        fn create() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "yssbi-duckdb-export-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn table_exports_csv_and_parquet_without_materializing_in_polars() {
        let directory = TestDirectory::create();
        let database_path = directory.path().join("source.duckdb");
        let csv_path = directory.path().join("output.csv");
        let parquet_path = directory.path().join("output.parquet");
        let connection = Connection::open(&database_path).expect("open DuckDB");
        connection
            .execute_batch(
                "CREATE TABLE \"sales\"\"2026\" (name VARCHAR, value BIGINT);\
                 INSERT INTO \"sales\"\"2026\" VALUES ('alpha', 1), ('beta', 2);",
            )
            .expect("seed table");
        drop(connection);

        export_duckdb_table(
            &database_path,
            "sales\"2026",
            &csv_path,
            DatabaseExportFormat::Csv,
        )
        .expect("export CSV");
        export_duckdb_table(
            &database_path,
            "sales\"2026",
            &parquet_path,
            DatabaseExportFormat::Parquet,
        )
        .expect("export Parquet");

        let csv = std::fs::read_to_string(csv_path).expect("read CSV");
        assert_eq!(
            csv.lines().collect::<Vec<_>>(),
            ["name,value", "alpha,1", "beta,2"]
        );
        assert!(
            std::fs::metadata(parquet_path)
                .expect("Parquet metadata")
                .len()
                > 0
        );
    }

    #[test]
    fn open_failures_retain_the_export_phase() {
        let directory = TestDirectory::create();
        let error = export_duckdb_table(
            directory.path(),
            "table",
            &directory.path().join("output.csv"),
            DatabaseExportFormat::Csv,
        )
        .expect_err("a directory cannot be opened as a DuckDB database");

        assert_eq!(error.phase(), DuckDbExportPhase::Open);
    }
}
