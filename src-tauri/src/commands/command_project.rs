use crate::event::EventProject;
use crate::event::{emit_project_event, Event};
use crate::frontend::FrontendError;
use crate::log::LogLevel;
use crate::log_app;
use crate::project::{load_project_from_file, save_project_to_file, ProjectData, ProjectState};
use tauri::{AppHandle, State};

use serde_json::Value;

/// 获取当前项目数据
#[tauri::command]
pub fn get_project_data(state: State<ProjectState>) -> crate::schema::ProjectDataDTO {
    let data = state.get_data();

    log_app!(
        LogLevel::Info,
        "[command.get_project_data] ProjectData: {}",
        data.info()
    );

    crate::schema::ProjectDataDTO::from(&data)
}

#[tauri::command]
pub fn set_project_data(
    app: AppHandle,
    state: State<ProjectState>,
    data: ProjectData,
    path: Option<String>,
    emit_event: Option<bool>,
) -> Result<(), String> {
    log_app!(
        LogLevel::Info,
        "[command.set_project_data] ProjectData: {}",
        data.info()
    );

    state.set_data(data.clone());

    if let Some(p) = path.clone() {
        log_app!(
            LogLevel::Info,
            "[command.set_project_data] Set path to: {}",
            p
        );
        state.set_path(Some(p));
    }

    if emit_event.unwrap_or(false) {
        log_app!(
            LogLevel::Info,
            "[command.set_project_data] Emitting ProjectLoaded event"
        );
        emit_project_event(
            &app,
            Event::Project(EventProject::ProjectLoaded { data, path }),
        );
    }

    Ok(())
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

/// 加载项目（从状态管理层）
#[tauri::command]
pub fn load_project_to_state(
    app: AppHandle,
    state: State<ProjectState>,
    path: String,
) -> Result<(), FrontendError> {
    log_app!(
        LogLevel::Info,
        "[command.load_project] Loading project from: {}",
        path
    );

    let project_data = load_project_from_file(&path)?;

    log_app!(
        LogLevel::Info,
        "[command.load_project] Project loaded: {}",
        project_data.info()
    );

    state.set_data(project_data.clone());
    state.set_path(Some(path.clone()));
    emit_project_event(
        &app,
        Event::Project(EventProject::ProjectLoaded {
            data: project_data.clone(),
            path: Some(path),
        }),
    );
    Ok(())
}

#[tauri::command]
pub fn save_project_from_state(
    app: AppHandle,
    state: State<ProjectState>,
    path: String,
) -> Result<(), FrontendError> {
    log_app!(
        LogLevel::Info,
        "[command.save_project] Saving project to: {}",
        path
    );

    let mut project_data = state.get_data();
    project_data.update_metadata();

    save_project_to_file(&project_data, &path)?;
    state.set_path(Some(path.clone()));

    emit_project_event(&app, Event::Project(EventProject::ProjectSaved { path }));
    Ok(())
}

/// 新建项目（清空当前状态）
#[tauri::command]
pub fn new_project(app: AppHandle, state: State<ProjectState>) -> Result<(), String> {
    log_app!(LogLevel::Info, "[command.new_project] Creating new project");

    state.clear();
    emit_project_event(&app, Event::Project(EventProject::ProjectCleared));
    Ok(())
}

/// @deprecated 使用 save_project_from_state 替代
#[tauri::command]
pub fn save_project(_path: String, _data: Value) -> Result<(), String> {
    Err("Deprecated: use save_project_from_state instead".to_string())
}

/// @deprecated 使用 load_project_to_state 替代
#[tauri::command]
pub fn load_project(_path: String) -> Result<Value, String> {
    Err("Deprecated: use load_project_to_state instead".to_string())
}

/// @deprecated 使用 set_project_data 替代
#[tauri::command]
pub fn parse_project(_data: Value) -> Result<Value, String> {
    Err("Deprecated: use set_project_data instead".to_string())
}

/// @deprecated 使用 set_project_data 替代
#[tauri::command]
pub fn serialize_project(_data: Value) -> Result<Value, String> {
    Err("Deprecated: use get_project_data instead".to_string())
}

/// 执行项目（待实现）
#[tauri::command]
pub fn execute_project() -> Result<Value, String> {
    Err("Not yet implemented".to_string())
}
