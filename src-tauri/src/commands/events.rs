//! Events 子图 CRUD 命令

use crate::project::SubGraphData;
use crate::state::{emit_project_event, ProjectEvent, ProjectState};
use std::collections::HashMap;
use tauri::{AppHandle, State};
use tauri_plugin_log::log::info;

/// 获取所有事件子图
#[tauri::command]
pub fn get_events(state: State<'_, ProjectState>) -> HashMap<String, SubGraphData> {
    let events = state.get_events();
    info!("[get_events] Returning {} events", events.len());
    events
}

/// 获取单个事件子图
#[tauri::command]
pub fn get_event(state: State<'_, ProjectState>, id: String) -> Option<SubGraphData> {
    let event = state.get_event(&id);
    info!("[get_event] id={}, found={}", id, event.is_some());
    event
}

/// 创建事件子图
#[tauri::command]
pub fn create_event(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    info!(
        "[create_event] id={}, name={}, nodes={}",
        id,
        data.name,
        data.nodes.len()
    );
    let result = state.create_event(id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::EventCreated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 更新事件子图
#[tauri::command]
pub fn update_event(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    info!(
        "[update_event] id={}, name={}, nodes={}",
        id,
        data.name,
        data.nodes.len()
    );
    let result = state.update_event(&id, data)?;
    emit_project_event(
        &app,
        ProjectEvent::EventUpdated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 删除事件子图
#[tauri::command]
pub fn delete_event(app: AppHandle, state: State<'_, ProjectState>, id: String) -> Result<(), String> {
    state.delete_event(&id)?;
    emit_project_event(&app, ProjectEvent::EventDeleted { id });
    Ok(())
}
