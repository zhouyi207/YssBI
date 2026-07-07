use std::path::{Path, PathBuf};

pub use crate::application::database_schema::name_from_path;
use crate::application::database_schema::{
    DatabaseSchemaSnapshot, column_info_from_duckdb, database_display_name, extract_database_schema,
};
use crate::database::{
    DatabaseDecl, DatabaseEngine, DatabaseEngineSql, DatabaseInstance, DatabaseState,
    drop_data_table, ingest_csv_to_duckdb, ingest_dataframe_to_duckdb, ingest_excel_to_duckdb,
    ingest_parquet_to_duckdb, read_table_meta, sql_reader, write_display_name,
};
use crate::project::{
    ProjectState, project_root_from_path, relative_project_duckdb_path, unique_name,
};
use crate::schema::{ColumnInfoDTO, DatabaseEngineDTO};
use serde::Serialize;
use uuid::Uuid;
use yss_sci::api::database::{EditHistory, EditState};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadDatabaseResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnInfoDTO>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseMetaResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnInfoDTO>,
}

pub fn load_database(
    state: &ProjectState,
    engine: DatabaseEngineDTO,
) -> Result<LoadDatabaseResult, String> {
    match engine {
        DatabaseEngineDTO::Csv {
            path,
            delimiter,
            has_header,
            infer_schema_length,
        } => load_csv_via_duckdb(state, path, delimiter, has_header, infer_schema_length),
        DatabaseEngineDTO::Parquet { path, columns } => {
            load_parquet_via_duckdb(state, path, columns)
        }
        DatabaseEngineDTO::Excel { path, sheet } => load_excel_via_duckdb(state, path, sheet),
        DatabaseEngineDTO::Sql {
            engine,
            connection_string,
            table,
        } => {
            let engine_sql = DatabaseEngineSql::try_from(engine)
                .map_err(|e| format!("Invalid SQL engine config: {}", e))?;
            load_sql_via_duckdb(state, engine_sql, connection_string, table)
        }
        DatabaseEngineDTO::DuckDb { .. } => Err(
            "DuckDb datasets are discovered from the project store; reopen the project to refresh"
                .into(),
        ),
        DatabaseEngineDTO::InMemory { .. } => {
            Err("InMemory datasets cannot be loaded via load_database".into())
        }
    }
}

fn load_csv_via_duckdb(
    state: &ProjectState,
    csv_path: String,
    delimiter: char,
    has_header: bool,
    infer_schema_length: Option<usize>,
) -> Result<LoadDatabaseResult, String> {
    let (id, table, duckdb_abs, relative_path) = prepare_duckdb_ingest_paths(state)?;

    let meta = ingest_csv_to_duckdb(
        Path::new(&csv_path),
        &duckdb_abs,
        &table,
        delimiter,
        has_header,
        infer_schema_length,
    )?;

    register_duckdb_instance(
        state,
        id,
        table,
        name_from_path(&csv_path),
        meta,
        duckdb_abs,
        relative_path,
    )
}

fn load_parquet_via_duckdb(
    state: &ProjectState,
    parquet_path: String,
    columns: Option<Vec<String>>,
) -> Result<LoadDatabaseResult, String> {
    let (id, table, duckdb_abs, relative_path) = prepare_duckdb_ingest_paths(state)?;

    let meta = ingest_parquet_to_duckdb(
        Path::new(&parquet_path),
        &duckdb_abs,
        &table,
        columns.as_deref(),
    )?;

    register_duckdb_instance(
        state,
        id,
        table,
        name_from_path(&parquet_path),
        meta,
        duckdb_abs,
        relative_path,
    )
}

fn load_excel_via_duckdb(
    state: &ProjectState,
    excel_path: String,
    sheet: String,
) -> Result<LoadDatabaseResult, String> {
    let (id, table, duckdb_abs, relative_path) = prepare_duckdb_ingest_paths(state)?;

    let meta = ingest_excel_to_duckdb(Path::new(&excel_path), &sheet, &duckdb_abs, &table)?;

    register_duckdb_instance(
        state,
        id,
        table,
        name_from_path(&excel_path),
        meta,
        duckdb_abs,
        relative_path,
    )
}

fn prepare_duckdb_ingest_paths(
    state: &ProjectState,
) -> Result<(String, String, PathBuf, String), String> {
    let project_path = state
        .get_path()
        .ok_or_else(|| "请先打开或创建项目后再导入数据".to_string())?;
    let project_root = project_root_from_path(&project_path);
    crate::project::ensure_project_database_dir(&project_root).map_err(|e| e.to_string())?;

    let id = format!("db-{}", Uuid::new_v4());
    let table = id.clone();
    let relative_path = relative_project_duckdb_path();
    let duckdb_abs = project_root.join(&relative_path);
    Ok((id, table, duckdb_abs, relative_path))
}

fn register_duckdb_instance(
    state: &ProjectState,
    id: String,
    table: String,
    base_name: String,
    meta: crate::database::DuckDbTableMeta,
    duckdb_abs: PathBuf,
    relative_path: String,
) -> Result<LoadDatabaseResult, String> {
    let name = unique_database_name(state, &base_name);
    write_display_name(&duckdb_abs, &table, &name)?;

    let columns = column_info_from_duckdb(&meta.columns);
    let column_count = columns.len();
    let row_count = meta.row_count;

    let engine_domain = DatabaseEngine::DuckDb {
        path: relative_path,
        table: table.clone(),
    };

    let decl = DatabaseDecl {
        id: id.clone(),
        engine: engine_domain,
        schema_version: 1,
        required: false,
        name: Some(name.clone()),
    };

    let instance = DatabaseInstance {
        decl,
        state: DatabaseState::DuckDb {
            duckdb_path: duckdb_abs.to_string_lossy().to_string(),
            table,
            row_count,
            columns: meta.columns,
            history: EditHistory::new(),
        },
    };
    state.add_database(instance);

    Ok(LoadDatabaseResult {
        id,
        name,
        row_count,
        column_count,
        columns,
    })
}

fn load_sql_via_duckdb(
    state: &ProjectState,
    engine: DatabaseEngineSql,
    connection_string: String,
    table: String,
) -> Result<LoadDatabaseResult, String> {
    let mut df = sql_reader::read_table_to_dataframe(&engine, &connection_string, &table)?;
    let (id, table_id, duckdb_abs, relative_path) = prepare_duckdb_ingest_paths(state)?;
    let meta = ingest_dataframe_to_duckdb(&mut df, &duckdb_abs, &table_id)?;
    let base_name = table.clone();
    register_duckdb_instance(
        state,
        id,
        table_id,
        base_name,
        meta,
        duckdb_abs,
        relative_path,
    )
}

pub fn bind_duckdb_instance(decl: &DatabaseDecl, project_root: Option<&Path>) -> DatabaseInstance {
    let DatabaseEngine::DuckDb { path, table, .. } = &decl.engine else {
        unreachable!("bind_duckdb_instance expects DuckDb engine");
    };

    let state = match project_root {
        Some(root) => {
            let abs = root.join(path);
            match read_table_meta(&abs, table) {
                Ok(meta) => DatabaseState::DuckDb {
                    duckdb_path: abs.to_string_lossy().to_string(),
                    table: table.clone(),
                    row_count: meta.row_count,
                    columns: meta.columns,
                    history: EditHistory::new(),
                },
                Err(error) => DatabaseState::Failed { error },
            }
        }
        None => DatabaseState::Failed {
            error: "Project path not set; cannot bind DuckDB database".into(),
        },
    };

    DatabaseInstance {
        decl: decl.clone(),
        state,
    }
}

pub fn get_database_meta(state: &ProjectState, id: &str) -> Result<DatabaseMetaResult, String> {
    let store = state.project_store.read().unwrap();
    let db = store.databases.get(id).ok_or("Database not found")?;

    match extract_database_schema(db) {
        DatabaseSchemaSnapshot::Ready {
            name,
            columns,
            row_count,
            column_count,
        } => Ok(DatabaseMetaResult {
            id: id.to_string(),
            name,
            row_count,
            column_count,
            columns,
        }),
        DatabaseSchemaSnapshot::Failed { name, error } => {
            Err(format!("Database '{name}' failed to load: {error}"))
        }
    }
}

fn unique_database_name(state: &ProjectState, base_name: &str) -> String {
    let store = state.project_store.read().unwrap();
    let existing: Vec<String> = store
        .databases
        .values()
        .map(database_display_name)
        .collect();
    unique_name::unique_name(base_name, existing.iter().map(|s| s.as_str()))
}

pub fn rename_database(state: &ProjectState, id: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Dataset name cannot be empty".into());
    }

    {
        let store = state.project_store.read().unwrap();
        if !store.databases.contains_key(id) {
            return Err("Database not found".into());
        }
        let duplicate = store
            .databases
            .iter()
            .any(|(other_id, db)| other_id != id && database_display_name(db) == name);
        if duplicate {
            return Err(format!("Dataset name '{name}' already exists"));
        }
    }

    let engine = {
        let mut store = state.project_store.write().unwrap();
        let db = store.databases.get_mut(id).ok_or("Database not found")?;
        db.decl.name = Some(name.to_string());
        db.decl.engine.clone()
    };

    {
        let mut data = state.project_data.write().unwrap();
        if let Some(decl) = data.databases.get_mut(id) {
            decl.name = Some(name.to_string());
        }
    }

    if let Some((relative_path, table)) = engine.duckdb_table() {
        if let Some(project_path) = state.get_path() {
            let root = project_root_from_path(&project_path);
            let abs = root.join(relative_path);
            write_display_name(&abs, table, name)?;
        }
    }

    state.invalidate_graph_runtime();
    Ok(())
}

pub fn remove_duckdb_table_if_needed(engine: &DatabaseEngine, project_root: Option<&Path>) {
    let Some((relative_path, table)) = engine.duckdb_table() else {
        return;
    };
    let Some(root) = project_root else {
        return;
    };
    let abs = root.join(relative_path);
    let _ = drop_data_table(&abs, table);
}

/// Persist in-memory edits into the project's DuckDB table (`project.duckdb`).
/// DuckDB-backed datasets transition back to `DatabaseState::DuckDb` after a successful save.
pub fn save_database_changes(state: &ProjectState, id: &str) -> Result<EditState, String> {
    let project_root = state.get_path().map(|p| project_root_from_path(&p));
    state.with_database_mut(id, |db| db.save_changes(project_root.as_deref()))
}
