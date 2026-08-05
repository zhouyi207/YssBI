use std::path::PathBuf;

use yssbi_lib::application::database::bind_duckdb_instance;
use yssbi_lib::database::{
    DatabaseAccess, DatabaseState, ingest_csv_to_duckdb, ingest_parquet_to_duckdb,
    query_page_to_dataframe, write_display_name,
};
use yssbi_lib::node_system::document::{
    DatabaseResourceKey, OperationId, ResourceKey, ResourceRevision,
};
use yssbi_lib::project::{
    ProjectInstanceId, ProjectState, discover_databases_from_root, project_duckdb_abs,
};

fn database_authority(
    state: &ProjectState,
    database_id: &str,
) -> (ProjectInstanceId, ResourceRevision) {
    let session = state
        .capture_project_session()
        .expect("capture project session");
    let index = state
        .read_project_index(&session.instance_id)
        .expect("read authoritative project index");
    assert_eq!(index.project_instance_id, session.instance_id.as_str());
    let revision = index
        .databases
        .iter()
        .find(|database| database.id == database_id)
        .expect("database authority")
        .revision;
    (session.instance_id, revision)
}

fn setup_iris_duckdb_project() -> (PathBuf, String) {
    let project_root = PathBuf::from(format!(
        "target/test_project_duckdb_{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&project_root);
    ProjectState::try_new()
        .expect("initialize built-in node system")
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

/// CSV 导入后写入 DuckDB，并可预览/执行读取。
#[test]
fn test_csv_ingest_to_duckdb_and_preview() {
    println!("\n=== 测试 CSV ingest → DuckDB ===");

    let (project_root, db_id) = setup_iris_duckdb_project();
    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    let decl = databases.get(&db_id).expect("decl");

    let mut db_instance = bind_duckdb_instance(decl, Some(project_root.as_path()));
    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

    let preview = db_instance
        .access(DatabaseAccess::Preview)
        .expect("preview");
    assert_eq!(preview.dataframe.height(), 100);
    assert!(preview.dataframe.width() >= 5);

    let full = db_instance
        .access(DatabaseAccess::Execution)
        .expect("execution");
    assert_eq!(full.dataframe.height(), 150);

    assert!(project_duckdb_abs(&project_root).is_file());

    let _ = std::fs::remove_dir_all(&project_root);
}

/// Phase 2：分页与 schema 不触发整表 Loaded。
#[test]
fn test_duckdb_query_page_and_schema_without_full_load() {
    let (project_root, db_id) = setup_iris_duckdb_project();
    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    let decl = databases.get(&db_id).expect("decl");
    let mut db_instance = bind_duckdb_instance(decl, Some(project_root.as_path()));

    let schema = db_instance.data_schema().expect("schema");
    assert!(schema.columns.len() >= 5);
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
    let state = ProjectState::try_new().expect("initialize built-in node system");
    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    assert_eq!(databases.len(), 1);
    assert_eq!(
        databases.get(&db_id).and_then(|d| d.name.as_deref()),
        Some("iris")
    );
    state.activate_project_from_path(&project_root).unwrap();

    let mut store = state.project_store.write().unwrap();
    let db = store.databases.get_mut(&db_id).expect("database in store");
    assert!(matches!(db.state, DatabaseState::DuckDb { .. }));

    let page = db.query_page(0, 20).expect("page after reload");
    assert_eq!(page.height(), 20);

    let _ = std::fs::remove_dir_all(&project_root);
}

/// 同一 project.duckdb 可承载多张表。
#[test]
fn test_single_project_duckdb_multiple_tables() {
    let project_root = PathBuf::from(format!(
        "target/test_project_multi_{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&project_root);
    ProjectState::try_new()
        .expect("initialize built-in node system")
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
        databases.get("db-a").and_then(|d| d.name.as_deref()),
        Some("iris-a")
    );
    assert_eq!(
        databases.get("db-b").and_then(|d| d.name.as_deref()),
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

    let parquet_path = PathBuf::from(format!("target/test_iris_{}.parquet", uuid::Uuid::new_v4()));
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

    let duckdb_path = PathBuf::from(format!(
        "target/test_parquet_{}.duckdb",
        uuid::Uuid::new_v4()
    ));
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
    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

    let _ = std::fs::remove_dir_all(&project_root);
}

/// Phase 5：`save_database_changes` 将编辑写回 `project.duckdb`，重开可恢复。
#[test]
fn test_edit_save_persists_to_duckdb() {
    use yssbi_lib::application::database::save_database_changes;

    let (project_root, db_id) = setup_iris_duckdb_project();
    let state = ProjectState::try_new().expect("initialize built-in node system");
    state.activate_project_from_path(&project_root).unwrap();

    let (project_instance_id, edit_expected_revision) = database_authority(&state, &db_id);
    let edit_operation_id = OperationId::new();
    let edited = state
        .with_database_mut(
            &project_instance_id,
            &db_id,
            edit_expected_revision,
            edit_operation_id,
            |db| db.edit_cell(0, "sepal_length", serde_json::json!(999.0), None),
        )
        .expect("edit");
    assert_eq!(
        edited.mutation.project_instance_id,
        project_instance_id.as_str()
    );
    assert_eq!(edited.mutation.operation_id, edit_operation_id);
    assert_eq!(edited.mutation.deltas.len(), 1);
    let edit_delta = &edited.mutation.deltas[0];
    assert_eq!(
        edit_delta.resource,
        ResourceKey::Database(DatabaseResourceKey(format!("databases/{db_id}").into()))
    );
    assert_eq!(edit_delta.from_revision, edit_expected_revision);
    assert_eq!(edit_delta.to_revision, edit_expected_revision.next());
    assert_eq!(edit_delta.caused_by, Some(edit_operation_id));

    let (save_project_instance_id, save_expected_revision) = database_authority(&state, &db_id);
    assert_eq!(save_project_instance_id, project_instance_id);
    assert_eq!(save_expected_revision, edit_delta.to_revision);
    let save_operation_id = OperationId::new();
    let saved = save_database_changes(
        &state,
        &save_project_instance_id,
        &db_id,
        save_expected_revision,
        save_operation_id,
    )
    .expect("save");
    assert_eq!(
        saved.mutation.project_instance_id,
        project_instance_id.as_str()
    );
    assert_eq!(saved.mutation.operation_id, save_operation_id);
    assert_eq!(saved.mutation.deltas.len(), 1);
    let save_delta = &saved.mutation.deltas[0];
    assert_eq!(
        save_delta.resource,
        ResourceKey::Database(DatabaseResourceKey(format!("databases/{db_id}").into()))
    );
    assert_eq!(save_delta.from_revision, save_expected_revision);
    assert_eq!(save_delta.to_revision, save_expected_revision.next());
    assert_eq!(save_delta.caused_by, Some(save_operation_id));
    assert!(!saved.data.can_undo);
    assert!(!saved.data.can_redo);
    assert!(!saved.data.is_modified);
    assert_eq!(saved.data.undo_count, 0);
    assert_eq!(saved.data.redo_count, 0);
    let (_, committed_revision) = database_authority(&state, &db_id);
    assert_eq!(committed_revision, save_delta.to_revision);
    drop(state);

    let databases = discover_databases_from_root(project_root.as_path()).expect("discover");
    let decl = databases.get(&db_id).expect("decl");
    let mut db_instance = bind_duckdb_instance(decl, Some(project_root.as_path()));
    assert!(matches!(db_instance.state, DatabaseState::DuckDb { .. }));

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
