use serde_json::Value;
use tauri::{AppHandle, State};
use crate::project::ProjectState;
use crate::variable::VariableDefinition;
use crate::log::log_sys;
use crate::event::{emit_project_event, Event, EventVariable};

/// 创建变量（统一接口，支持全局和局部变量）
#[tauri::command]
pub fn create_variable(
    variable: Value,
    state: State<ProjectState>,
    app: AppHandle,
) -> Result<String, String> {
    log_sys::info!("[create_variable] Creating variable: {:?}", variable);
    
    // 解析变量对象
    let mut var_def: VariableDefinition = serde_json::from_value(variable)
        .map_err(|e| format!("Failed to parse variable: {}", e))?;
    
    // 始终由后端分配 ID（前端不得分配）
    let var_id = format!("var_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
    var_def.id = var_id.clone();
    
    // 添加到项目数据
    {
        let mut project_data = state.project_data.write().unwrap();
        project_data.variables.insert(var_id.clone(), var_def.clone());
    }
    
    log_sys::info!("[create_variable] Variable created with ID: {}", var_id);
    Ok(var_id)
}

/// 获取变量（统一接口）
#[tauri::command]
pub fn get_variable(
    id: String,
    state: State<ProjectState>,
) -> Result<Value, String> {
    log_sys::info!("[get_variable] Getting variable: {}", id);
    
    let project_data = state.project_data.read().unwrap();
    
    let var_def = project_data.variables.get(&id)
        .ok_or_else(|| format!("Variable not found: {}", id))?;
    
    let value = serde_json::to_value(var_def)
        .map_err(|e| format!("Failed to serialize variable: {}", e))?;
    
    log_sys::info!("[get_variable] Variable retrieved: {}", id);
    Ok(value)
}

/// 更新变量（统一接口）
#[tauri::command]
pub fn update_variable(
    id: String,
    variable: Value,
    state: State<ProjectState>,
) -> Result<(), String> {
    log_sys::info!("[update_variable] Updating variable: {}", id);
    
    // 解析变量对象
    let var_def: VariableDefinition = serde_json::from_value(variable)
        .map_err(|e| format!("Failed to parse variable: {}", e))?;
    
    // 更新项目数据
    {
        let mut project_data = state.project_data.write().unwrap();
        
        if !project_data.variables.contains_key(&id) {
            return Err(format!("Variable not found: {}", id));
        }
        
        project_data.variables.insert(id.clone(), var_def);
    }
    
    log_sys::info!("[update_variable] Variable updated: {}", id);
    Ok(())
}

/// 删除变量（统一接口）
#[tauri::command]
pub fn delete_variable(
    id: String,
    state: State<ProjectState>,
) -> Result<(), String> {
    log_sys::info!("[delete_variable] Deleting variable: {}", id);
    
    // 从项目数据中删除
    {
        let mut project_data = state.project_data.write().unwrap();
        
        if project_data.variables.remove(&id).is_none() {
            return Err(format!("Variable not found: {}", id));
        }
    }
    
    log_sys::info!("[delete_variable] Variable deleted: {}", id);
    Ok(())
}
