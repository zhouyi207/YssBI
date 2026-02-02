//! DataFrame 相关命令

use crate::project::DataFrameData;
use crate::state::{emit_project_event, ProjectEvent, ProjectState};
use chrono::Utc;
use polars::prelude::*;
use tauri::{AppHandle, State};
use tauri_plugin_log::log::info;

/// 从 CSV 导入数据
#[tauri::command]
pub async fn import_csv(
    app: AppHandle,
    state: State<'_, ProjectState>,
    path: String,
) -> Result<DataFrameData, String> {
    info!("[import_csv] Importing from: {}", path);

    // 使用 Polars 读取 CSV
    let df = CsvReadOptions::default()
        .with_has_header(true)
        .with_infer_schema_length(Some(100))
        .try_into_reader_with_file_path(Some(path.clone().into()))
        .map_err(|e| format!("Failed to open CSV: {}", e))?
        .finish()
        .map_err(|e| format!("Failed to parse CSV: {}", e))?;

    let id = format!("df_{:x}", Utc::now().timestamp_nanos_opt().unwrap_or(0));
    let df_data = state.add_dataframe(id.clone(), df, Some(path))?;

    // 通知所有窗口
    emit_project_event(
        &app,
        ProjectEvent::DataFrameCreated {
            id,
            data: df_data.clone(),
        },
    );

    Ok(df_data)
}

/// 删除数据帧
#[tauri::command]
pub fn delete_dataframe(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
) -> Result<(), String> {
    info!("[delete_dataframe] id={}", id);
    state.delete_dataframe(&id)?;
    emit_project_event(&app, ProjectEvent::DataFrameDeleted { id });
    Ok(())
}

/// 创建数据帧（手动创建）
#[tauri::command]
pub fn create_dataframe(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: DataFrameData,
) -> Result<DataFrameData, String> {
    info!("[create_dataframe] id={}, name={}", id, data.name);
    let result = state.create_dataframe(id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::DataFrameCreated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 获取数据帧行数据
#[tauri::command]
pub fn get_dataframe_rows(
    state: State<'_, ProjectState>,
    id: String,
    offset: usize,
    limit: usize,
) -> Result<Vec<Vec<serde_json::Value>>, String> {
    let df_store = state.df_store.read().unwrap();
    let df = df_store
        .get(&id)
        .ok_or_else(|| format!("DataFrame '{}' not found in memory", id))?;

    let height = df.height();
    if offset >= height {
        return Ok(vec![]);
    }

    let actual_limit = std::cmp::min(limit, height - offset);
    let slice = df.slice(offset as i64, actual_limit);

    let mut rows = Vec::new();
    for i in 0..slice.height() {
        let mut row = Vec::new();
        for col_idx in 0..slice.width() {
            let val = slice.get_columns()[col_idx].get(i).unwrap();
            let json_val = match val {
                polars::prelude::AnyValue::Null => serde_json::Value::Null,
                polars::prelude::AnyValue::Boolean(b) => serde_json::Value::Bool(b),
                polars::prelude::AnyValue::String(s) => serde_json::Value::String(s.to_string()),
                polars::prelude::AnyValue::StringOwned(s) => {
                    serde_json::Value::String(s.to_string())
                }
                polars::prelude::AnyValue::Int8(v) => serde_json::json!(v),
                polars::prelude::AnyValue::Int16(v) => serde_json::json!(v),
                polars::prelude::AnyValue::Int32(v) => serde_json::json!(v),
                polars::prelude::AnyValue::Int64(v) => serde_json::json!(v),
                polars::prelude::AnyValue::UInt8(v) => serde_json::json!(v),
                polars::prelude::AnyValue::UInt16(v) => serde_json::json!(v),
                polars::prelude::AnyValue::UInt32(v) => serde_json::json!(v),
                polars::prelude::AnyValue::UInt64(v) => serde_json::json!(v),
                polars::prelude::AnyValue::Float32(v) => serde_json::json!(v),
                polars::prelude::AnyValue::Float64(v) => serde_json::json!(v),
                _ => serde_json::Value::String(format!("{:?}", val)),
            };
            row.push(json_val);
        }
        rows.push(row);
    }

    Ok(rows)
}
