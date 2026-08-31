use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use polars::prelude::DataType;
use yss_database_contract::DatabaseEngineSql;

use crate::dataframe::ColumnKind;
use crate::{list_tables, read_table_to_dataframe, runtime, sqlite};

static NEXT_DATABASE: AtomicU64 = AtomicU64::new(0);

struct TestDatabase {
    path: PathBuf,
}

impl TestDatabase {
    fn new(label: &str) -> Self {
        let sequence = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "yssbi-sql-source-{label}-{}-{sequence}.sqlite",
            std::process::id()
        ));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn create(&self, sql: &'static str) {
        runtime::run(sqlite::execute_fixture_sql(self.path.clone(), sql))
            .expect("create SQLite fixture");
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("sqlite-shm"));
        let _ = std::fs::remove_file(self.path.with_extension("sqlite-wal"));
    }
}

fn sqlite_engine(auto_create: bool) -> DatabaseEngineSql {
    DatabaseEngineSql::Sqlite { auto_create }
}

#[test]
fn sqlite_source_preserves_typed_values_binary_and_quoted_names() {
    let database = TestDatabase::new("typed-values");
    database.create(
        "CREATE TABLE \"odd\"\"table\" (\
             signed INTEGER, ratio REAL, label TEXT, enabled BOOLEAN, payload BLOB\
         ); \
         INSERT INTO \"odd\"\"table\" VALUES \
             (-7, 1.5, 'alpha', TRUE, X'00FF'), \
             (NULL, NULL, NULL, NULL, NULL);",
    );
    let path = database.path().to_string_lossy();

    let tables = list_tables(&sqlite_engine(false), &path).expect("list SQLite tables");
    assert_eq!(tables, vec!["odd\"table"]);

    let frame = read_table_to_dataframe(&sqlite_engine(false), &path, "odd\"table")
        .expect("read quoted SQLite table");
    assert_eq!(frame.height(), 2);
    assert_eq!(frame.width(), 5);
    assert_eq!(
        frame.column("signed").unwrap().i64().unwrap().get(0),
        Some(-7)
    );
    assert_eq!(
        frame.column("ratio").unwrap().f64().unwrap().get(0),
        Some(1.5)
    );
    assert_eq!(
        frame.column("label").unwrap().str().unwrap().get(0),
        Some("alpha")
    );
    assert_eq!(
        frame.column("enabled").unwrap().bool().unwrap().get(0),
        Some(true)
    );
    assert_eq!(
        frame.column("payload").unwrap().binary().unwrap().get(0),
        Some(&[0, 255][..])
    );
    assert_eq!(frame.column("signed").unwrap().null_count(), 1);
    assert_eq!(frame.column("payload").unwrap().null_count(), 1);
}

#[test]
fn empty_sqlite_table_retains_column_names_and_declared_dtypes() {
    let database = TestDatabase::new("empty-schema");
    database.create("CREATE TABLE records (id INTEGER, label TEXT, payload BLOB);");
    let path = database.path().to_string_lossy();

    let frame = read_table_to_dataframe(&sqlite_engine(false), &path, "records")
        .expect("read empty SQLite table");

    assert_eq!(frame.height(), 0);
    assert_eq!(frame.get_column_names(), &["id", "label", "payload"]);
    assert_eq!(frame.column("id").unwrap().dtype(), &DataType::Int64);
    assert_eq!(frame.column("label").unwrap().dtype(), &DataType::String);
    assert_eq!(frame.column("payload").unwrap().dtype(), &DataType::Binary);
}

#[test]
fn sqlite_auto_create_is_explicit_and_missing_read_only_sources_stay_missing() {
    let database = TestDatabase::new("auto-create");
    let path = database.path().to_string_lossy().into_owned();
    assert!(!database.path().exists());

    let error = list_tables(&sqlite_engine(false), &path)
        .expect_err("read-only missing SQLite source must fail");
    assert_eq!(error.to_string(), "failed to connect to SQLite");
    assert!(!database.path().exists());

    let tables = list_tables(&sqlite_engine(true), &path).expect("create requested SQLite source");
    assert!(tables.is_empty());
    assert!(database.path().exists());
}

#[test]
fn sync_api_is_safe_when_called_from_an_existing_tokio_runtime() {
    let database = TestDatabase::new("nested-runtime");
    database.create("CREATE TABLE records (value INTEGER);");
    let path = database.path().to_string_lossy().into_owned();

    let tables = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move { list_tables(&sqlite_engine(false), &path) })
        .expect("list tables from inside Tokio");
    assert_eq!(tables, vec!["records"]);
}

#[test]
fn engine_identifier_quoting_escapes_only_its_own_delimiter() {
    assert_eq!(
        crate::postgres::quote_identifier_for_test("a\"b`c"),
        "\"a\"\"b`c\""
    );
    assert_eq!(
        crate::mysql::quote_identifier_for_test("a`b\"c"),
        "`a``b\"c`"
    );
    assert_eq!(
        crate::sqlite::quote_identifier_for_test("a\"b`c"),
        "\"a\"\"b`c\""
    );
}

#[test]
fn engine_metadata_maps_to_exact_supported_polars_kinds() {
    let postgres = crate::postgres::column_specs(vec![
        ("internal_char".into(), "CHAR".into()),
        ("small".into(), "INT2".into()),
        ("regular".into(), "INT4".into()),
        ("large".into(), "INT8".into()),
        ("object_id".into(), "OID".into()),
    ])
    .expect("supported PostgreSQL metadata");
    assert_eq!(
        postgres
            .iter()
            .map(|column| column.kind)
            .collect::<Vec<_>>(),
        vec![
            ColumnKind::Int8,
            ColumnKind::Int16,
            ColumnKind::Int32,
            ColumnKind::Int64,
            ColumnKind::UInt32,
        ]
    );

    let mysql = crate::mysql::column_specs(vec![
        ("tiny".into(), "TINYINT".into()),
        ("unsigned_big".into(), "BIGINT UNSIGNED".into()),
        ("document".into(), "JSON".into()),
        ("flags".into(), "SET".into()),
        ("bits".into(), "BIT".into()),
    ])
    .expect("supported MySQL metadata");
    assert_eq!(
        mysql.iter().map(|column| column.kind).collect::<Vec<_>>(),
        vec![
            ColumnKind::Int8,
            ColumnKind::UInt64,
            ColumnKind::String,
            ColumnKind::String,
            ColumnKind::Binary,
        ]
    );

    let error = crate::postgres::column_specs(vec![("amount".into(), "NUMERIC".into())])
        .expect_err("unsupported source types must fail before row decoding");
    assert!(matches!(
        error,
        crate::SqlSourceError::UnsupportedColumnType { .. }
    ));
}
