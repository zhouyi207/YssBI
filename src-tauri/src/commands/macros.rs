//! Macros 子图 CRUD 命令

use crate::project::SubGraphData;
use crate::state::{emit_project_event, ProjectEvent, ProjectState};
use std::collections::HashMap;
use tauri::{AppHandle, State};

/// 获取所有宏子图
#[tauri::command]
pub fn get_macros(state: State<'_, ProjectState>) -> HashMap<String, SubGraphData> {
    state.get_macros()
}

/// 获取单个宏子图
#[tauri::command]
pub fn get_macro(state: State<'_, ProjectState>, id: String) -> Option<SubGraphData> {
    state.get_macro(&id)
}

/// 创建宏子图
#[tauri::command]
pub fn create_macro(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    let result = state.create_macro(id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::MacroCreated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 更新宏子图
#[tauri::command]
pub fn update_macro(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    let result = state.update_macro(&id, data)?;
    emit_project_event(
        &app,
        ProjectEvent::MacroUpdated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 删除宏子图
#[tauri::command]
pub fn delete_macro(app: AppHandle, state: State<'_, ProjectState>, id: String) -> Result<(), String> {
    state.delete_macro(&id)?;
    emit_project_event(&app, ProjectEvent::MacroDeleted { id });
    Ok(())
}
