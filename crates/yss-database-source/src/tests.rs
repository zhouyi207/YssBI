use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use arrow::array::{Array, BinaryArray, BooleanArray, Float64Array, Int64Array, StringArray};
use arrow::datatypes::DataType;
use arrow::record_batch::{RecordBatch, RecordBatchReader};
use yss_database_contract::DatabaseEngineSql;

use crate::batch::ColumnKind;
use crate::{list_tables, read_table_batches, runtime, sqlite};

fn control() -> yss_relational_contract::RelationControl {
    yss_relational_contract::RelationControl {
        cancellation: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(15),
        max_input_bytes: 16 * 1024 * 1024,
    }
}
fn read_table(
    engine: &DatabaseEngineSql,
    path: &str,
    table: &str,
) -> Result<RecordBatch, crate::SqlSourceError> {
    let reader = read_table_batches(engine, path, table, control())?;
    let schema = reader.schema();
    let batches = reader.collect::<Result<Vec<_>, _>>()?;
    arrow::compute::concat_batches(&schema, &batches).map_err(Into::into)
}

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

    let frame =
        read_table(&sqlite_engine(false), &path, "odd\"table").expect("read quoted SQLite table");
    assert_eq!(frame.num_rows(), 2);
    assert_eq!(frame.num_columns(), 5);
    assert_eq!(
        frame
            .column_by_name("signed")
            .unwrap()
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .iter()
            .next()
            .unwrap(),
        Some(-7)
    );
    assert_eq!(
        frame
            .column_by_name("ratio")
            .unwrap()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .iter()
            .next()
            .unwrap(),
        Some(1.5)
    );
    assert_eq!(
        frame
            .column_by_name("label")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
            .iter()
            .next()
            .unwrap(),
        Some("alpha")
    );
    assert_eq!(
        frame
            .column_by_name("enabled")
            .unwrap()
            .as_any()
            .downcast_ref::<BooleanArray>()
            .unwrap()
            .iter()
            .next()
            .unwrap(),
        Some(true)
    );
    assert_eq!(
        frame
            .column_by_name("payload")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap()
            .iter()
            .next()
            .unwrap(),
        Some(&[0, 255][..])
    );
    assert_eq!(frame.column_by_name("signed").unwrap().null_count(), 1);
    assert_eq!(frame.column_by_name("payload").unwrap().null_count(), 1);
}

#[test]
fn empty_sqlite_table_retains_column_names_and_declared_dtypes() {
    let database = TestDatabase::new("empty-schema");
    database.create("CREATE TABLE records (id INTEGER, label TEXT, payload BLOB);");
    let path = database.path().to_string_lossy();

    let frame =
        read_table(&sqlite_engine(false), &path, "records").expect("read empty SQLite table");

    assert_eq!(frame.num_rows(), 0);
    assert_eq!(
        frame
            .schema()
            .fields()
            .iter()
            .map(|field| field.name().as_str())
            .collect::<Vec<_>>(),
        vec!["id", "label", "payload"]
    );
    assert_eq!(
        frame.column_by_name("id").unwrap().data_type(),
        &DataType::Int64
    );
    assert_eq!(
        frame.column_by_name("label").unwrap().data_type(),
        &DataType::Utf8
    );
    assert_eq!(
        frame.column_by_name("payload").unwrap().data_type(),
        &DataType::Binary
    );
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
fn engine_metadata_maps_to_exact_supported_arrow_kinds() {
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

#[test]
fn streaming_sql_emits_bounded_batches_and_rejects_a_late_incompatible_value() {
    let database = TestDatabase::new("late-decode");
    database.create("CREATE TABLE records (value INTEGER); WITH RECURSIVE seq(n) AS (SELECT 0 UNION ALL SELECT n+1 FROM seq WHERE n<100000) INSERT INTO records SELECT n FROM seq; INSERT INTO records VALUES('bad integer');");
    let mut reader = read_table_batches(
        &sqlite_engine(false),
        &database.path().to_string_lossy(),
        "records",
        control(),
    )
    .unwrap();
    assert_eq!(reader.next().unwrap().unwrap().num_rows(), 50_000);
    assert_eq!(reader.next().unwrap().unwrap().num_rows(), 50_000);
    assert!(reader.next().unwrap().is_err());
    assert!(reader.next().is_none());
}

#[test]
fn dropping_or_cancelling_a_backpressured_sql_reader_releases_its_worker() {
    let database = TestDatabase::new("cancel");
    database.create("CREATE TABLE records (value INTEGER); WITH RECURSIVE seq(n) AS (SELECT 0 UNION ALL SELECT n+1 FROM seq WHERE n<300000) INSERT INTO records SELECT n FROM seq;");
    let control = control();
    let mut reader = read_table_batches(
        &sqlite_engine(false),
        &database.path().to_string_lossy(),
        "records",
        control.clone(),
    )
    .unwrap();
    assert_eq!(reader.next().unwrap().unwrap().num_rows(), 50_000);
    control.cancellation.store(true, Ordering::Release);
    let start = std::time::Instant::now();
    assert!(reader.next().unwrap().is_err());
    drop(reader);
    assert!(start.elapsed() < std::time::Duration::from_secs(3));
}
