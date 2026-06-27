use crate::project::ProjectState;
use crate::schema::DatabaseEngineDTO;
use tauri::State;

mod types;

use types::dataframe_to_row_matrix;

async fn run_on_blocking_pool<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce() -> Result<R, String> + Send + 'static,
    R: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseRowsPayload {
    rows: Vec<Vec<serde_json::Value>>,
    row_ids: Vec<i64>,
}

#[tauri::command]
pub async fn load_database(
    state: State<'_, ProjectState>,
    engine: DatabaseEngineDTO,
) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    let result =
        run_on_blocking_pool(move || crate::application::database::load_database(&state, engine))
            .await?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sqlite_tables(db_path: String) -> Result<Vec<String>, String> {
    run_on_blocking_pool(move || {
        use crate::database::DatabaseEngineSql;
        crate::database::sql_reader::list_tables(
            &DatabaseEngineSql::Sqlite { auto_create: false },
            &db_path,
        )
    })
    .await
}

#[tauri::command]
pub async fn list_sql_tables(
    engine: String,
    connection_string: String,
) -> Result<Vec<String>, String> {
    run_on_blocking_pool(move || {
        use crate::database::DatabaseEngineSql;
        let engine_enum = match engine.as_str() {
            "postgres" | "postgresql" => DatabaseEngineSql::Postgres { ssl: true },
            "mysql" | "mariadb" => DatabaseEngineSql::Mysql {
                charset: "utf8mb4".to_string(),
            },
            _ => return Err(format!("Unsupported SQL engine: {}", engine)),
        };
        crate::database::sql_reader::list_tables(&engine_enum, &connection_string)
    })
    .await
}

#[tauri::command]
pub async fn list_excel_sheets(file_path: String) -> Result<Vec<String>, String> {
    run_on_blocking_pool(move || crate::database::excel_reader::list_sheets(&file_path)).await
}

#[tauri::command]
pub fn get_database_meta(
    state: State<ProjectState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let result = crate::application::database::get_database_meta(state.inner(), &id)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_database(state: State<'_, ProjectState>, id: String) -> Result<(), String> {
    let state = state.inner().clone();
    run_on_blocking_pool(move || {
        state.delete_database(&id);
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn rename_database(state: State<ProjectState>, id: String, name: String) -> Result<(), String> {
    crate::application::database::rename_database(state.inner(), &id, &name)
}

#[tauri::command]
pub fn get_database_rows(
    state: State<ProjectState>,
    id: String,
    offset: usize,
    limit: usize,
) -> Result<serde_json::Value, String> {
    let page = state.with_database_mut(&id, |db| {
        db.query_page_with_rowids(offset, limit)
            .map_err(|e| format!("Failed to query database page: {}", e))
    })?;

    let payload = DatabaseRowsPayload {
        rows: dataframe_to_row_matrix(&page.dataframe),
        row_ids: page.row_ids,
    };
    serde_json::to_value(payload).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_column_stats(
    state: State<ProjectState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let stats = state.with_database_mut(&id, |db| {
        db.compute_column_stats()
            .map_err(|e| format!("Failed to compute column stats: {}", e))
    })?;

    serde_json::to_value(stats).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_column_distribution(
    state: State<ProjectState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let dists = state.with_database_mut(&id, |db| {
        db.compute_column_distributions()
            .map_err(|e| format!("Failed to compute column distribution: {}", e))
    })?;

    serde_json::to_value(dists).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_dataset_overview(
    state: State<ProjectState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let overview = state.with_database_mut(&id, |db| {
        db.compute_dataset_overview()
            .map_err(|e| format!("Failed to compute dataset overview: {}", e))
    })?;

    serde_json::to_value(overview).map_err(|e| e.to_string())
}

// ==================== Edit Commands ====================

#[tauri::command]
pub fn edit_cell(
    state: State<ProjectState>,
    id: String,
    row: usize,
    col_name: String,
    value: serde_json::Value,
    row_id: Option<i64>,
) -> Result<serde_json::Value, String> {
    let edit_state =
        state.with_database_mut(&id, |db| db.edit_cell(row, &col_name, value, row_id))?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_row(
    state: State<ProjectState>,
    id: String,
    index: Option<usize>,
) -> Result<serde_json::Value, String> {
    let edit_state = state.with_database_mut(&id, |db| db.add_row(index))?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_rows(
    state: State<ProjectState>,
    id: String,
    indices: Vec<usize>,
    row_ids: Option<Vec<i64>>,
) -> Result<serde_json::Value, String> {
    let edit_state =
        state.with_database_mut(&id, |db| db.delete_rows(&indices, row_ids.as_deref()))?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_column(
    state: State<ProjectState>,
    id: String,
    name: String,
    dtype: String,
) -> Result<serde_json::Value, String> {
    let edit_state = state.with_database_mut(&id, |db| db.add_column(&name, &dtype))?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_column(
    state: State<ProjectState>,
    id: String,
    name: String,
) -> Result<serde_json::Value, String> {
    let edit_state = state.with_database_mut(&id, |db| db.delete_column(&name))?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cast_column(
    state: State<ProjectState>,
    id: String,
    col_name: String,
    new_dtype: String,
    force: Option<bool>,
) -> Result<serde_json::Value, String> {
    let edit_state = state.with_database_mut(&id, |db| {
        db.cast_column(&col_name, &new_dtype, force.unwrap_or(false))
    })?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_column(
    state: State<ProjectState>,
    id: String,
    old_name: String,
    new_name: String,
) -> Result<serde_json::Value, String> {
    let edit_state = state.with_database_mut(&id, |db| db.rename_column(&old_name, &new_name))?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn undo_edit(state: State<ProjectState>, id: String) -> Result<serde_json::Value, String> {
    let edit_state = state.with_database_mut(&id, |db| db.undo_edit())?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn redo_edit(state: State<ProjectState>, id: String) -> Result<serde_json::Value, String> {
    let edit_state = state.with_database_mut(&id, |db| db.redo_edit())?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_database_changes(
    state: State<ProjectState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let edit_state = crate::application::database::save_database_changes(state.inner(), &id)?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}

/// Export the current dataset view (including unsaved in-memory edits) to an external file.
/// Use `save_database_changes` to persist edits into `project.duckdb`.
#[tauri::command]
pub fn export_database(
    state: State<ProjectState>,
    id: String,
    path: String,
    format: String,
) -> Result<(), String> {
    let view = state
        .access_database(&id, crate::database::DatabaseAccess::Execution)
        .map_err(|e| format!("Failed to access database: {}", e))?;

    let mut df = view.dataframe;
    yss_sci::api::database::export_dataframe(&mut df, &path, &format)
}

#[tauri::command]
pub fn get_edit_state(state: State<ProjectState>, id: String) -> Result<serde_json::Value, String> {
    let edit_state = state.with_database_mut(&id, |db| Ok(db.edit_state()))?;
    serde_json::to_value(edit_state).map_err(|e| e.to_string())
}
