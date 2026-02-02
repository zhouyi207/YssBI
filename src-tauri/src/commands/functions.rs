//! Functions 子图 CRUD 命令

use crate::project::SubGraphData;
use crate::state::{emit_project_event, ProjectEvent, ProjectState};
use std::collections::HashMap;
use tauri::{AppHandle, State};

/// 获取所有函数子图
#[tauri::command]
pub fn get_functions(state: State<'_, ProjectState>) -> HashMap<String, SubGraphData> {
    state.get_functions()
}

/// 获取单个函数子图
#[tauri::command]
pub fn get_function(state: State<'_, ProjectState>, id: String) -> Option<SubGraphData> {
    state.get_function(&id)
}

/// 创建函数子图
#[tauri::command]
pub fn create_function(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    let result = state.create_function(id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::FunctionCreated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 更新函数子图
#[tauri::command]
pub fn update_function(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    let result = state.update_function(&id, data)?;
    emit_project_event(
        &app,
        ProjectEvent::FunctionUpdated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 删除函数子图
#[tauri::command]
pub fn delete_function(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
) -> Result<(), String> {
    state.delete_function(&id)?;
    emit_project_event(&app, ProjectEvent::FunctionDeleted { id });
    Ok(())
}
