//! 变量管理相关命令（全局变量和局部变量）

use crate::schema::VariableDefinition;
use crate::state::{emit_project_event, ProjectEvent, ProjectState};
use std::collections::HashMap;
use tauri::{AppHandle, State};

// ==================== Global Variables CRUD ====================

/// 获取所有全局变量
#[tauri::command]
pub fn get_global_variables(state: State<'_, ProjectState>) -> HashMap<String, VariableDefinition> {
    state.get_global_variables()
}

/// 获取单个全局变量
#[tauri::command]
pub fn get_global_variable(state: State<'_, ProjectState>, id: String) -> Option<VariableDefinition> {
    state.get_global_variable(&id)
}

/// 创建全局变量
#[tauri::command]
pub fn create_global_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: VariableDefinition,
) -> Result<VariableDefinition, String> {
    let result = state.create_global_variable(id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::GlobalVariableCreated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 更新全局变量
#[tauri::command]
pub fn update_global_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: VariableDefinition,
) -> Result<VariableDefinition, String> {
    let result = state.update_global_variable(&id, data)?;
    emit_project_event(
        &app,
        ProjectEvent::GlobalVariableUpdated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 删除全局变量
#[tauri::command]
pub fn delete_global_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
) -> Result<(), String> {
    state.delete_global_variable(&id)?;
    emit_project_event(&app, ProjectEvent::GlobalVariableDeleted { id });
    Ok(())
}

// ==================== Local Variables CRUD ====================

/// 获取子图的局部变量
#[tauri::command]
pub fn get_local_variables(
    state: State<'_, ProjectState>,
    subgraph_id: String,
) -> Result<HashMap<String, VariableDefinition>, String> {
    state.get_local_variables(&subgraph_id)
}

/// 创建局部变量
#[tauri::command]
pub fn create_local_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    variable_id: String,
    data: VariableDefinition,
) -> Result<VariableDefinition, String> {
    let result = state.create_local_variable(&subgraph_id, variable_id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::LocalVariableCreated {
            subgraph_id,
            variable_id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 更新局部变量
#[tauri::command]
pub fn update_local_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    variable_id: String,
    data: VariableDefinition,
) -> Result<VariableDefinition, String> {
    let result = state.update_local_variable(&subgraph_id, &variable_id, data)?;
    emit_project_event(
        &app,
        ProjectEvent::LocalVariableUpdated {
            subgraph_id,
            variable_id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 删除局部变量
#[tauri::command]
pub fn delete_local_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    variable_id: String,
) -> Result<(), String> {
    state.delete_local_variable(&subgraph_id, &variable_id)?;
    emit_project_event(
        &app,
        ProjectEvent::LocalVariableDeleted {
            subgraph_id,
            variable_id,
        },
    );
    Ok(())
}

// ==================== Unified Create Variable ====================

/// 统一的变量创建接口
#[tauri::command]
pub fn create_variable(
    state: State<'_, ProjectState>,
    subgraph_id: Option<String>,
    name: Option<String>,
    data_type: Option<String>,
) -> Result<VariableDefinition, String> {
    state.create_variable(subgraph_id, name, data_type)
}
