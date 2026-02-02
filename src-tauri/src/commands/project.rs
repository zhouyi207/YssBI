//! 项目管理相关命令

use crate::project::ProjectData;
use crate::state::{emit_project_event, ProjectEvent, ProjectState};
use tauri::{AppHandle, State};
use tauri_plugin_log::log::info;

/// 获取当前项目状态
#[tauri::command]
pub fn get_project_state(state: State<'_, ProjectState>) -> ProjectData {
    let data = state.get_data();
    info!(
        "[get_project_state] events={}, functions={}, macros={}, globalVars={}",
        data.events.len(),
        data.functions.len(),
        data.macros.len(),
        data.global_variables.len()
    );
    // 打印每个 event 的详细信息
    for (id, event) in &data.events {
        info!(
            "[get_project_state] Event '{}': name='{}', nodes={}",
            id,
            event.name,
            event.nodes.len()
        );
    }
    data
}

/// 获取当前项目路径
#[tauri::command]
pub fn get_project_path(state: State<'_, ProjectState>) -> Option<String> {
    let path = state.get_current_path();
    info!("[get_project_path] path={:?}", path);
    path
}

/// 新建项目（清空当前状态）
#[tauri::command]
pub fn new_project(app: AppHandle, state: State<'_, ProjectState>) -> Result<(), String> {
    info!("[new_project] Clearing project state");
    state.clear();
    emit_project_event(&app, ProjectEvent::ProjectCleared);
    Ok(())
}

/// 加载项目（从状态管理器）
#[tauri::command]
pub fn load_project_to_state(
    app: AppHandle,
    state: State<'_, ProjectState>,
    path: String,
) -> Result<ProjectData, String> {
    info!("[load_project_to_state] Loading from path: {}", path);
    let project = crate::project::load_project_from_file(&path)?;
    info!(
        "[load_project_to_state] Loaded: global_vars={}, events={}, functions={}, macros={}",
        project.global_variables.len(),
        project.events.len(),
        project.functions.len(),
        project.macros.len()
    );

    // 记录加载的变量详情
    for (id, var) in &project.global_variables {
        info!(
            "[load_project_to_state] Global Variable '{}': name={}, type={:?}",
            id, var.name, var.data_type
        );
    }

    state.set_data(project.clone());
    state.set_current_path(Some(path.clone()));
    emit_project_event(
        &app,
        ProjectEvent::ProjectLoaded {
            data: project.clone(),
            path: Some(path),
        },
    );
    Ok(project)
}

/// 保存项目（从状态管理器）
#[tauri::command]
pub fn save_project_from_state(
    app: AppHandle,
    state: State<'_, ProjectState>,
    path: String,
) -> Result<(), String> {
    info!("[save_project_from_state] Saving to path: {}", path);
    let mut project = state.get_data();
    project.update_metadata();
    crate::project::save_project_to_file(&project, &path)?;
    state.set_current_path(Some(path.clone()));
    emit_project_event(&app, ProjectEvent::ProjectSaved { path });
    Ok(())
}

/// 设置项目数据（用于前端批量同步）
#[tauri::command]
pub fn set_project_data(
    app: AppHandle,
    state: State<'_, ProjectState>,
    data: ProjectData,
    path: Option<String>,
    emit_event: Option<bool>, // 是否触发事件，默认 false
) -> Result<(), String> {
    info!(
        "[set_project_data] Receiving data: events={}, functions={}, macros={}, global_vars={}",
        data.events.len(),
        data.functions.len(),
        data.macros.len(),
        data.global_variables.len()
    );
    // 打印每个 event 的详细信息
    for (id, event) in &data.events {
        info!(
            "[set_project_data] Event '{}': name='{}', nodes={}",
            id,
            event.name,
            event.nodes.len()
        );
    }
    state.set_data(data.clone());
    if let Some(p) = path.clone() {
        state.set_current_path(Some(p));
    }
    info!("[set_project_data] Data stored successfully");

    // 只在明确要求时才触发事件（避免重复触发）
    if emit_event.unwrap_or(false) {
        info!("[set_project_data] Emitting ProjectLoaded event");
        emit_project_event(&app, ProjectEvent::ProjectLoaded { data, path });
    }
    Ok(())
}

// ==================== 兼容旧接口的项目文件命令 ====================

/// 保存项目到指定路径（兼容旧接口）
#[tauri::command]
pub fn save_project(path: String, project_json: String) -> Result<(), String> {
    let mut project: ProjectData = serde_json::from_str(&project_json)
        .map_err(|e| format!("Failed to parse project data: {}", e))?;

    // 更新元数据时间戳
    project.update_metadata();

    crate::project::save_project_to_file(&project, &path)
}

/// 从指定路径加载项目（兼容旧接口）
#[tauri::command]
pub fn load_project(path: String) -> Result<ProjectData, String> {
    crate::project::load_project_from_file(&path)
}

/// 解析项目 JSON（不涉及文件操作）
#[tauri::command]
pub fn parse_project(json: String) -> Result<ProjectData, String> {
    ProjectData::from_json(&json)
}

/// 序列化项目为 JSON（不涉及文件操作）
#[tauri::command]
pub fn serialize_project(project: ProjectData) -> Result<String, String> {
    project.to_json()
}
