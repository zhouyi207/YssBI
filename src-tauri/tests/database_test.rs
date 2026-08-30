use std::path::PathBuf;
use std::sync::Arc;

use polars::prelude::*;
use yss_database_contract::{DatabaseDecl, DatabaseEngine, DatabaseExportFormat, DatabaseId};
use yss_project_identity::OperationId;
use yssbi_lib::database::{
    DatabaseInstance, DatabaseState, EditHistory, MAX_DELETE_COLUMN_SNAPSHOT_ROWS,
    MAX_IN_MEMORY_EDIT_ROWS, bind_duckdb_instance, ingest_csv_to_duckdb, ingest_parquet_to_duckdb,
    query_page_to_dataframe, read_table_meta, write_display_name,
};
use yssbi_lib::project::{ProjectState, discover_databases_from_root, project_duckdb_abs};

fn loaded_instance(dataframe: DataFrame) -> DatabaseInstance {
    DatabaseInstance {
        decl: DatabaseDecl {
            id: DatabaseId::from_existing("test".into()),
            engine: DatabaseEngine::InMemory {
                name: "test".into(),
            },
            schema_version: 1,
            required: false,
            name: "Test".into(),
        },
        state: DatabaseState::Loaded {
            original: Arc::new(dataframe.clone()),
            dataframe: Arc::new(dataframe),
            history: EditHistory::new(),
        },
    }
}

fn test_output_path(name: String) -> PathBuf {
    let directory = PathBuf::from("target");
    std::fs::create_dir_all(&directory).expect("create database test output directory");
    directory.join(name)
}

fn duckdb_instance(duckdb_path: &PathBuf, table: &str) -> DatabaseInstance {
    let meta = read_table_meta(duckdb_path, table).unwrap();
    DatabaseInstance {
        decl: DatabaseDecl {
            id: DatabaseId::from_existing(table.into()),
            engine: DatabaseEngine::DuckDb {
                path: duckdb_path.to_string_lossy().into_owned(),
                table: table.into(),
            },
            schema_version: 1,
            required: false,
            name: table.into(),
        },
        state: DatabaseState::DuckDb {
            duckdb_path: duckdb_path.to_string_lossy().into_owned(),
            table: table.into(),
            row_count: meta.row_count,
            columns: meta.columns,
            history: EditHistory::new(),
        },
    }
}

#[test]
fn add_column_rejects_unknown_dtype_without_history() {
    let mut database = loaded_instance(df!("value" => [1_i64]).unwrap());

    let error = database.add_column("invalid", "Mystery").unwrap_err();

    assert!(error.contains("Mystery"));
    assert_eq!(database.list_column_names().unwrap(), vec!["value"]);
    assert!(!database.edit_state().can_undo);
}

#[test]
fn edit_cell_rejects_lossy_integer_json_numbers() {
    let mut database = loaded_instance(
        df!(
            "signed" => [7_i8],
            "unsigned" => [9_u8],
        )
        .unwrap(),
    );

    for (column, value) in [
        ("signed", serde_json::json!(1.5)),
        ("signed", serde_json::json!(128)),
        ("unsigned", serde_json::json!(-1)),
    ] {
        assert!(database.edit_cell(0, column, value, None).is_err());
        assert!(!database.edit_state().can_undo);
    }

    let page = database.query_page(0, 1).unwrap();
    assert_eq!(page.column("signed").unwrap().i8().unwrap().get(0), Some(7));
    assert_eq!(
        page.column("unsigned").unwrap().u8().unwrap().get(0),
        Some(9)
    );
}

#[test]
fn polars_delete_column_undo_restores_dtype_and_data() {
    let mut database = loaded_instance(
        df!(
            "keep" => [10_i64, 20, 30],
            "removed" => [Some(1_i32), None, Some(-2)],
        )
        .unwrap(),
    );

    database.delete_column("removed").unwrap();
    database.undo_edit().unwrap();

    let page = database.query_page(0, 3).unwrap();
    let restored = page.column("removed").unwrap();
    assert_eq!(restored.dtype(), &DataType::Int32);
    assert_eq!(
        restored.i32().unwrap().into_iter().collect::<Vec<_>>(),
        vec![Some(1), None, Some(-2)]
    );
}

#[test]
fn duckdb_edit_quotes_identifiers_separately_from_string_literals() {
    let duckdb_path = test_output_path(format!(
        "test_database_quotes_{}.duckdb",
        uuid::Uuid::new_v4()
    ));
    let table = "table\"with'quotes";
    let column = "value\"with'quotes";
    let conn = duckdb::Connection::open(&duckdb_path).unwrap();
    conn.execute_batch(
        r#"CREATE TABLE "table""with'quotes" ("value""with'quotes" VARCHAR);
           INSERT INTO "table""with'quotes" VALUES ('before');"#,
    )
    .unwrap();
    drop(conn);

    let mut database = duckdb_instance(&duckdb_path, table);

    database
        .edit_cell(0, column, serde_json::json!("O'Reilly\\path"), None)
        .unwrap();

    let page = database.query_page(0, 1).unwrap();
    assert_eq!(
        page.column(column).unwrap().str().unwrap().get(0),
        Some("O'Reilly\\path")
    );

    let _ = std::fs::remove_file(duckdb_path);
}

#[test]
fn duckdb_force_cast_is_rejected_without_mutation() {
    let duckdb_path = test_output_path(format!(
        "test_database_force_{}.duckdb",
        uuid::Uuid::new_v4()
    ));
    let conn = duckdb::Connection::open(&duckdb_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE force_cast (value VARCHAR); INSERT INTO force_cast VALUES ('1');",
    )
    .unwrap();
    drop(conn);

    let mut database = duckdb_instance(&duckdb_path, "force_cast");

    let error = database.cast_column("value", "Int64", true).unwrap_err();

    assert!(error.to_lowercase().contains("force"));
    assert!(!database.edit_state().can_undo);
    let page = database.query_page(0, 1).unwrap();
    assert_eq!(page.column("value").unwrap().dtype(), &DataType::String);
    assert_eq!(
        page.column("value").unwrap().str().unwrap().get(0),
        Some("1")
    );

    let _ = std::fs::remove_file(duckdb_path);
}

#[test]
fn duckdb_delete_column_undo_restores_dtype_and_data() {
    let duckdb_path = test_output_path(format!(
        "test_database_delete_column_{}.duckdb",
        uuid::Uuid::new_v4()
    ));
    let table = "delete_column";
    let conn = duckdb::Connection::open(&duckdb_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE delete_column (keep BIGINT, removed INTEGER);\n\
         INSERT INTO delete_column VALUES (1, 7), (2, NULL), (3, -2);",
    )
    .unwrap();
    drop(conn);

    let mut database = duckdb_instance(&duckdb_path, table);

    database.delete_column("removed").unwrap();
    database.undo_edit().unwrap();

    let page = database.query_page(0, 3).unwrap();
    let restored = page.column("removed").unwrap();
    assert_eq!(restored.dtype(), &DataType::Int32);
    assert_eq!(
        restored.i32().unwrap().into_iter().collect::<Vec<_>>(),
        vec![Some(7), None, Some(-2)]
    );

    let _ = std::fs::remove_file(duckdb_path);
}

#[test]
fn duckdb_delete_column_over_snapshot_limit_is_rejected_without_history() {
    let duckdb_path = test_output_path(format!(
        "test_database_delete_limit_{}.duckdb",
        uuid::Uuid::new_v4()
    ));
    let table = "delete_limit";
    let row_count = MAX_DELETE_COLUMN_SNAPSHOT_ROWS + 1;
    let conn = duckdb::Connection::open(&duckdb_path).unwrap();
    conn.execute_batch(&format!(
        "CREATE TABLE {table} AS \
         SELECT i::BIGINT AS keep, i::INTEGER AS removed \
         FROM range({row_count}) AS rows(i);"
    ))
    .unwrap();
    drop(conn);

    let mut database = duckdb_instance(&duckdb_path, table);

    let error = database.delete_column("removed").unwrap_err();

    assert!(error.to_lowercase().contains("limit"));
    assert!(!database.edit_state().can_undo);
    assert!(
        database
            .list_column_names()
            .unwrap()
            .contains(&"removed".to_string())
    );

    let _ = std::fs::remove_file(duckdb_path);
}

#[test]
fn duckdb_storage_export_supports_large_quoted_tables_without_loading() {
    let duckdb_path = test_output_path(format!(
        "test_database_export_{}.duckdb",
        uuid::Uuid::new_v4()
    ));
    let table = "export\"table";
    let row_count = MAX_IN_MEMORY_EDIT_ROWS + 1;
    let conn = duckdb::Connection::open(&duckdb_path).unwrap();
    conn.execute_batch(&format!(
        "CREATE TABLE \"export\"\"table\" AS \
         SELECT i::BIGINT AS id FROM range({row_count}) AS rows(i);"
    ))
    .unwrap();
    drop(conn);

    let database = duckdb_instance(&duckdb_path, table);
    let csv_path = test_output_path(format!(
        "test_database_export_'_{}.csv",
        uuid::Uuid::new_v4()
    ));
    let parquet_path = csv_path.with_extension("parquet");
    std::fs::write(&csv_path, b"reserved").unwrap();
    std::fs::write(&parquet_path, b"reserved").unwrap();

    database
        .export_to_path(&csv_path, DatabaseExportFormat::Csv)
        .unwrap();
    database
        .export_to_path(&parquet_path, DatabaseExportFormat::Parquet)
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(&csv_path).unwrap().lines().count(),
        row_count + 1
    );
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let parquet_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM read_parquet(?)",
            [parquet_path.to_string_lossy().as_ref()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(parquet_rows as usize, row_count);
    assert!(matches!(database.state, DatabaseState::DuckDb { .. }));

    let _ = std::fs::remove_file(csv_path);
    let _ = std::fs::remove_file(parquet_path);
    let _ = std::fs::remove_file(duckdb_path);
}

#[test]
fn duckdb_delete_column_failed_undo_is_atomic_and_keeps_history() {
    let duckdb_path = test_output_path(format!(
        "test_database_delete_atomic_{}.duckdb",
        uuid::Uuid::new_v4()
    ));
    let table = "delete_atomic";
    let conn = duckdb::Connection::open(&duckdb_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE delete_atomic (keep BIGINT, removed INTEGER);\n\
         INSERT INTO delete_atomic VALUES (1, 7), (2, 8), (3, 9);",
    )
    .unwrap();
    drop(conn);

    let mut database = duckdb_instance(&duckdb_path, table);
    database.delete_column("removed").unwrap();

    let conn = duckdb::Connection::open(&duckdb_path).unwrap();
    conn.execute_batch(
        "DELETE FROM delete_atomic WHERE rowid = 1;\n\
         INSERT INTO delete_atomic (keep) VALUES (4);",
    )
    .unwrap();
    drop(conn);

    let error = database.undo_edit().unwrap_err();

    assert!(error.contains("rowid 1"));
    assert!(database.edit_state().can_undo);
    assert!(!database.edit_state().can_redo);
    let meta = read_table_meta(&duckdb_path, table).unwrap();
    assert!(meta.columns.iter().all(|column| column.name != "removed"));

    let _ = std::fs::remove_file(duckdb_path);
}

fn setup_iris_duckdb_project() -> (PathBuf, String) {
    let project_root = test_output_path(format!("test_project_duckdb_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&project_root);
    ProjectState::new()
        .create_project_transaction("Database test", &project_root, OperationId::new())
        .expect("create project fixture");

    let db_id = "db-test-iris";
    let duckdb_path = project_duckdb_abs(&project_root);

    ingest_csv_to_duckdb(
        PathBuf::from("tests/data/iris.csv").as_path(),
        &duckdb_path,
        db_id,
        ',',
        true,
        Some(100),
    )
    .expect("ingest csv");
    write_display_name(&duckdb_path, db_id, "iris").expect("write display name");

    (project_root, db_id.to_string())
}

/// Phase 2：分页与 schema 不触发整表 Loaded。
#[test]
fn test_duckdb_query_page_and_schema_without_full_load() {
    let (project_root, db_id) = setup_iris_duckdb_project();
    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    let decl = databases.get(&db_id).expect("decl");
    let mut db_instance = bind_duckdb_instance(decl, Some(project_root.as_path()));

    let columns = db_instance.list_column_names().expect("schema");
    assert!(columns.len() >= 5);
    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

    let page = db_instance.query_page(10, 5).expect("page");
    assert_eq!(page.height(), 5);
    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

    let direct = query_page_to_dataframe(&project_duckdb_abs(&project_root), &db_id, 20, 3)
        .expect("direct page");
    assert_eq!(direct.height(), 3);

    let _ = std::fs::remove_dir_all(&project_root);
}

/// Phase 4.5：打开项目时从 project.duckdb 枚举表并绑定。
#[test]
fn test_project_reload_discovers_duckdb_from_directory() {
    let (project_root, db_id) = setup_iris_duckdb_project();
    let state = ProjectState::new();
    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    assert_eq!(databases.len(), 1);
    assert_eq!(databases.get(&db_id).map(|d| d.name.as_ref()), Some("iris"));
    state.activate_project_from_path(&project_root).unwrap();

    let data = state.get_data().expect("project data after reload");
    let declaration = data
        .databases
        .get(&db_id)
        .expect("database in project data");
    assert_eq!(
        declaration.engine.duckdb_table(),
        Some(("database/project.duckdb", db_id.as_str())),
    );
    let mut database = bind_duckdb_instance(declaration, Some(project_root.as_path()));
    assert!(matches!(database.state, DatabaseState::DuckDb { .. }));

    let page = database.query_page(0, 20).expect("page after reload");
    assert_eq!(page.height(), 20);

    let _ = std::fs::remove_dir_all(&project_root);
}

/// 同一 project.duckdb 可承载多张表。
#[test]
fn test_single_project_duckdb_multiple_tables() {
    let project_root = test_output_path(format!("test_project_multi_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&project_root);
    ProjectState::new()
        .create_project_transaction("Multi database test", &project_root, OperationId::new())
        .expect("create project fixture");
    let duckdb_path = project_duckdb_abs(&project_root);
    let csv = PathBuf::from("tests/data/iris.csv");

    ingest_csv_to_duckdb(&csv, &duckdb_path, "db-a", ',', true, Some(100)).expect("ingest a");
    ingest_csv_to_duckdb(&csv, &duckdb_path, "db-b", ',', true, Some(100)).expect("ingest b");
    write_display_name(&duckdb_path, "db-a", "iris-a").expect("name a");
    write_display_name(&duckdb_path, "db-b", "iris-b").expect("name b");

    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    assert_eq!(databases.len(), 2);
    assert_eq!(
        databases.get("db-a").map(|d| d.name.as_ref()),
        Some("iris-a")
    );
    assert_eq!(
        databases.get("db-b").map(|d| d.name.as_ref()),
        Some("iris-b")
    );
    assert_eq!(
        databases.get("db-a").unwrap().engine.duckdb_table(),
        Some(("database/project.duckdb", "db-a"))
    );

    let _ = std::fs::remove_dir_all(&project_root);
}

/// Phase 1：Parquet ingest → DuckDB 表。
#[test]
fn test_parquet_ingest_to_duckdb() {
    use polars::prelude::*;

    let parquet_path = test_output_path(format!("test_iris_{}.parquet", uuid::Uuid::new_v4()));
    let csv_path = PathBuf::from("tests/data/iris.csv");
    let mut df = LazyCsvReader::new(PlRefPath::new(csv_path.to_string_lossy().as_ref()))
        .with_has_header(true)
        .finish()
        .expect("scan csv")
        .collect()
        .expect("collect csv");
    let file = std::fs::File::create(&parquet_path).expect("create parquet");
    ParquetWriter::new(file)
        .finish(&mut df)
        .expect("write parquet");

    let duckdb_path = test_output_path(format!("test_parquet_{}.duckdb", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_file(&duckdb_path);

    let meta = ingest_parquet_to_duckdb(&parquet_path, &duckdb_path, "db-parquet-test", None)
        .expect("ingest parquet");

    assert_eq!(meta.row_count, 150);
    assert!(meta.columns.len() >= 5);

    let _ = std::fs::remove_file(&parquet_path);
    let _ = std::fs::remove_file(&duckdb_path);
}

/// Phase 3：按列加载不触发整表 Loaded。
#[test]
fn test_duckdb_load_columns_without_full_load() {
    let (project_root, db_id) = setup_iris_duckdb_project();
    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    let decl = databases.get(&db_id).expect("decl");
    let mut db_instance = bind_duckdb_instance(decl, Some(project_root.as_path()));

    let series = db_instance
        .load_column_series("sepal_length")
        .expect("load column");
    assert_eq!(series.len(), 150);
    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

    let narrow = db_instance
        .load_columns(&["sepal_length", "sepal_width"])
        .expect("load columns");
    assert_eq!(narrow.height(), 150);
    assert_eq!(narrow.width(), 2);
    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

    let _ = std::fs::remove_dir_all(&project_root);
}

/// Phase 4：DuckDB 列统计/分布/概览不触发整表 Loaded。
#[test]
fn test_duckdb_analytics_without_full_load() {
    let (project_root, db_id) = setup_iris_duckdb_project();
    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    let decl = databases.get(&db_id).expect("decl");
    let mut db_instance = bind_duckdb_instance(decl, Some(project_root.as_path()));

    let stats = db_instance.compute_column_stats().expect("stats");
    assert!(stats.len() >= 5);
    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

    let dists = db_instance
        .compute_column_distributions()
        .expect("distributions");
    assert_eq!(dists.len(), stats.len());

    let overview = db_instance.compute_dataset_overview().expect("overview");
    assert_eq!(overview.size_shape.n_rows, 150);
    assert!(overview.size_shape.n_columns >= 5);
    assert_eq!(overview.size_shape.estimated_dataframe_memory_bytes, None);
    assert_eq!(overview.size_shape.duplicated_rows, None);
    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

    let _ = std::fs::remove_dir_all(&project_root);
}

/// Phase 5：DatabaseInstance 保存编辑后，重开可恢复。
#[test]
fn test_edit_save_persists_to_duckdb() {
    let (project_root, db_id) = setup_iris_duckdb_project();
    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    let decl = databases.get(&db_id).expect("decl");
    let mut db_instance = bind_duckdb_instance(decl, Some(project_root.as_path()));
    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

    db_instance
        .edit_cell(0, "sepal_length", serde_json::json!(999.0), None)
        .expect("edit");
    let saved = db_instance
        .save_changes(Some(project_root.as_path()))
        .expect("save");
    assert!(!saved.can_undo);
    assert!(!saved.can_redo);
    assert!(!saved.is_modified);
    drop(db_instance);

    let databases = discover_databases_from_root(project_root.as_path()).expect("rediscover");
    let decl = databases.get(&db_id).expect("rediscovered declaration");
    let mut db_instance = bind_duckdb_instance(decl, Some(project_root.as_path()));

    let page = db_instance.query_page(0, 1).expect("page");
    let val = page
        .column("sepal_length")
        .expect("column")
        .f64()
        .expect("f64")
        .get(0)
        .unwrap();
    assert!((val - 999.0).abs() < 1e-6);

    let _ = std::fs::remove_dir_all(&project_root);
}

/// Phase 6：SQL 编辑不触发 Loaded 整表物化。
#[test]
fn test_duckdb_sql_edit_without_full_load() {
    let (project_root, db_id) = setup_iris_duckdb_project();
    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    let decl = databases.get(&db_id).expect("decl");
    let mut db_instance = bind_duckdb_instance(decl, Some(project_root.as_path()));

    let page = db_instance.query_page_with_rowids(0, 1).expect("page");
    let row_id = page.row_ids[0];

    db_instance
        .edit_cell(0, "sepal_length", serde_json::json!(123.0), Some(row_id))
        .expect("sql edit");

    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

    let page2 = db_instance
        .query_page_with_rowids(0, 1)
        .expect("page after edit");
    let val = page2
        .dataframe
        .column("sepal_length")
        .expect("column")
        .f64()
        .expect("f64")
        .get(0)
        .unwrap();
    assert!((val - 123.0).abs() < 1e-6);

    let _ = std::fs::remove_dir_all(&project_root);
}

/// ingest / schema 不含物理行键列。
#[test]
fn test_duckdb_ingest_meta_has_no_physical_rowid_column() {
    let (project_root, db_id) = setup_iris_duckdb_project();
    let meta = yssbi_lib::database::read_table_meta(&project_duckdb_abs(&project_root), &db_id)
        .expect("meta");

    assert!(
        meta.columns.iter().all(|c| c.name != "_yssbi_rowid"),
        "schema must not contain physical rowid column: {:?}",
        meta.columns.iter().map(|c| &c.name).collect::<Vec<_>>()
    );

    let _ = std::fs::remove_dir_all(&project_root);
}

/// 删行 → undo 恢复 → redo 再删（DatabaseInstance 路径）。
#[test]
fn test_duckdb_delete_undo_redo() {
    let (project_root, db_id) = setup_iris_duckdb_project();
    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    let decl = databases.get(&db_id).expect("decl");
    let mut db_instance = bind_duckdb_instance(decl, Some(project_root.as_path()));

    let before = db_instance.query_page(0, 150).expect("page");
    assert_eq!(before.height(), 150);

    let page = db_instance.query_page_with_rowids(0, 1).expect("page");
    let row_id = page.row_ids[0];
    let original_val = page
        .dataframe
        .column("sepal_length")
        .expect("col")
        .f64()
        .expect("f64")
        .get(0)
        .unwrap();

    db_instance
        .delete_rows(&[0], Some(&[row_id]))
        .expect("delete");

    let after_delete = db_instance.query_page(0, 150).expect("page");
    assert_eq!(after_delete.height(), 149);

    db_instance.undo_edit().expect("undo");
    let after_undo = db_instance.query_page(0, 150).expect("page");
    assert_eq!(after_undo.height(), 150);
    let restored_exists = after_undo
        .column("sepal_length")
        .expect("col")
        .f64()
        .expect("f64")
        .into_iter()
        .flatten()
        .any(|v| (v - original_val).abs() < 1e-6);
    assert!(restored_exists, "undo should restore deleted row data");

    db_instance.redo_edit().expect("redo");
    let after_redo = db_instance.query_page(0, 150).expect("page");
    assert_eq!(after_redo.height(), 149);

    let _ = std::fs::remove_dir_all(&project_root);
}
