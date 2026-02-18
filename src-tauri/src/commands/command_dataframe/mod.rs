use crate::database::{DatabaseDecl, DatabaseInstance, DatabaseState};
use crate::project::ProjectState;
use crate::schema::DatabaseEngineDTO;
use polars::prelude::*;
use serde::Serialize;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

/// load_database 返回给前端的数据结构
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadDatabaseResult {
    id: String,
    name: String,
    row_count: usize,
    column_count: usize,
    columns: Vec<ColumnInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ColumnInfo {
    name: String,
    #[serde(rename = "type")]
    dtype: String,
}

/// 从路径提取文件名（不含扩展名）
fn name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed")
        .to_string()
}

/// 将 Polars DataType 转为前端可读字符串
fn dtype_to_string(dt: &DataType) -> String {
    format!("{:?}", dt)
}

#[tauri::command]
pub fn load_database(
    state: State<ProjectState>,
    engine: DatabaseEngineDTO,
) -> Result<serde_json::Value, String> {
    let engine_domain = crate::database::DatabaseEngine::try_from(engine.clone())
        .map_err(|e| format!("Invalid engine config: {}", e))?;

    let mut lazy_frame = engine_domain
        .build_lazy()
        .map_err(|e| format!("Failed to build lazy frame: {}", e))?;

    let schema = lazy_frame
        .collect_schema()
        .map_err(|e| format!("Failed to collect schema: {}", e))?;

    let columns: Vec<ColumnInfo> = schema
        .iter_names()
        .filter_map(|name| {
            schema.get(name).map(|dt| ColumnInfo {
                name: name.to_string(),
                dtype: dtype_to_string(dt),
            })
        })
        .collect();

    let column_count = columns.len();

    let row_count = {
        let count_df = lazy_frame
            .clone()
            .select([len()])
            .collect()
            .map_err(|e| format!("Failed to get row count: {}", e))?;
        count_df
            .get_columns()
            .first()
            .and_then(|s| s.u32().ok())
            .and_then(|ca| ca.get(0))
            .map(|v| v as usize)
            .unwrap_or(0)
    };

    let id = format!("db-{}", Uuid::new_v4());
    let name = match &engine {
        DatabaseEngineDTO::Csv { path, .. } => name_from_path(path),
        DatabaseEngineDTO::Parquet { path, .. } => name_from_path(path),
        _ => id.clone(),
    };

    let decl = DatabaseDecl {
        id: id.clone(),
        engine: engine_domain,
        schema_version: 1,
        required: false,
    };

    let instance = DatabaseInstance {
        decl: decl.clone(),
        state: DatabaseState::Lazy { lazy_frame },
    };

    state.add_database(instance);

    let result = LoadDatabaseResult {
        id: id.clone(),
        name,
        row_count,
        column_count,
        columns,
    };

    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_database_meta(
    state: State<ProjectState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let view = state
        .access_database(&id, crate::database::DatabaseAccess::Preview)
        .map_err(|e| format!("Failed to access database: {}", e))?;

    let df = &view.dataframe;
    let schema = df.schema();
    let columns: Vec<ColumnInfo> = schema
        .iter_names()
        .filter_map(|name| {
            schema.get(name).map(|dt| ColumnInfo {
                name: name.to_string(),
                dtype: dtype_to_string(dt),
            })
        })
        .collect();

    let (name, row_count) = {
        let store = state.project_store.read().unwrap();
        let db = store.databases.get(&id).ok_or("Database not found")?;
        let name = match &db.decl.engine {
            crate::database::DatabaseEngine::Csv { path, .. } => name_from_path(path),
            crate::database::DatabaseEngine::Parquet { path, .. } => name_from_path(path),
            crate::database::DatabaseEngine::InMemory { name } => name.clone(),
            _ => id.clone(),
        };
        let row_count = match &db.state {
            crate::database::DatabaseState::Loaded { dataframe } => dataframe.height(),
            crate::database::DatabaseState::Lazy { lazy_frame } => {
                let lf = lazy_frame.clone();
                drop(store);
                let count_df = lf
                    .select([len()])
                    .collect()
                    .map_err(|e| format!("Failed to get row count: {}", e))?;
                count_df
                    .get_columns()
                    .first()
                    .and_then(|s| s.u32().ok())
                    .and_then(|ca| ca.get(0))
                    .map(|v| v as usize)
                    .unwrap_or(0)
            }
            crate::database::DatabaseState::Failed { .. } => 0,
        };
        (name, row_count)
    };

    let result = serde_json::json!({
        "id": id,
        "name": name,
        "rowCount": row_count,
        "columnCount": columns.len(),
        "columns": columns,
    });

    Ok(result)
}

#[tauri::command]
pub fn delete_database(state: State<ProjectState>, id: String) -> Result<(), String> {
    state.delete_database(&id);
    Ok(())
}

#[tauri::command]
pub fn get_database_rows(
    state: State<ProjectState>,
    id: String,
    offset: usize,
    limit: usize,
) -> Result<serde_json::Value, String> {
    let view = state
        .access_database(&id, crate::database::DatabaseAccess::Execution)
        .map_err(|e| format!("Failed to access database: {}", e))?;

    let df = &view.dataframe;
    let total = df.height();
    let start = offset.min(total);
    let count = limit.min(total.saturating_sub(start));

    let sliced = df.slice(start as i64, count);

    let result: Vec<Vec<serde_json::Value>> = (0..sliced.height())
        .map(|row_idx| {
            sliced
                .get_columns()
                .iter()
                .map(|s| {
                    match s.get(row_idx) {
                        Ok(v) => polars_value_to_json(v),
                        Err(_) => serde_json::Value::Null,
                    }
                })
                .collect()
        })
        .collect();

    serde_json::to_value(result).map_err(|e| e.to_string())
}

fn polars_value_to_json(v: AnyValue<'_>) -> serde_json::Value {
    use polars::prelude::AnyValue;
    match v {
        AnyValue::Null => serde_json::Value::Null,
        AnyValue::Boolean(b) => serde_json::Value::Bool(b),
        AnyValue::String(s) => serde_json::Value::String(s.to_string()),
        AnyValue::Int64(i) => serde_json::Number::from_f64(i as f64)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(i.to_string())),
        AnyValue::UInt64(u) => serde_json::Number::from_f64(u as f64)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(u.to_string())),
        AnyValue::Float64(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        _ => serde_json::Value::String(format!("{:?}", v)),
    }
}
