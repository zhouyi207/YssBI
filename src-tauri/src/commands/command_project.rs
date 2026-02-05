use crate::event::EventProject;
use crate::event::{emit_project_event, Event};
use crate::log::LogLevel;
use crate::log_app;
use crate::project::{ProjectData, ProjectState};
use serde_json::Value;
use tauri::{AppHandle, State};

/// 获取当前项目数据
#[tauri::command]
pub fn get_project_data(state: State<ProjectState>) -> ProjectData {
    let data = state.get_data();

    log_app!(
        LogLevel::Info,
        "[command.get_project_data] ProjectData: {}",
        data.info()
    );

    data
}

/// 获取当前项目路径
#[tauri::command]
pub fn get_project_path(state: State<ProjectState>) -> Option<String> {
    let path = state.get_path();

    log_app!(
        LogLevel::Info,
        "[command.get_project_path] Path: {:?}",
        path
    );

    path
}

/// 新建项目（清空当前状态）
#[tauri::command]
pub fn new_project(app: AppHandle, state: State<ProjectState>) -> Result<(), String> {
    log_app!(LogLevel::Info, "[command.new_project] Creating new project");

    state.clear();
    emit_project_event(&app, Event::Project(EventProject::ProjectCleared));
    Ok(())
}

/// 加载项目（从状态管理层）
#[tauri::command]
pub fn load_project_to_state(_path: String, _state: State<ProjectState>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn save_project_from_state(_path: String, _state: State<ProjectState>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn set_project_data(_data: Value, _state: State<ProjectState>) -> Result<(), String> {
    Ok(())
}
