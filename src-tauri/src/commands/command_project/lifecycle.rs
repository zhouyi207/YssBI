use crate::error::AppError;
use crate::event::{Event, EventProject, emit_project_event};
use crate::execution::ResultSourceStore;
use crate::frontend::FrontendError;
use crate::log::LogLevel;
use crate::log_app;
use crate::project::{
    PROJECT_METADATA_FILE, ProjectData, ProjectRecord, ProjectRegistry, ProjectState,
    ProjectWatcherState, ensure_project_database_dir, load_project_from_file,
    normalize_existing_path, normalize_project_name, save_project_as_to_directory,
    save_project_to_file, validate_new_project_path as validate_new_project_path_impl,
};
use tauri::{AppHandle, State};

fn emit_project_loaded(app: &AppHandle, project_data: ProjectData, path: String) {
    emit_project_event(
        app,
        Event::Project(EventProject::ProjectLoaded {
            data: project_data,
            path: Some(path),
        }),
    );
}

fn start_project_watcher(app: &AppHandle, watcher: &ProjectWatcherState, path: &str) {
    if let Err(error) = watcher.watch_project(app.clone(), path) {
        tauri_plugin_log::log::warn!("Failed to start project watcher: {}", error);
    }
}

/// 加载项目（从状态管理层）
#[tauri::command]
pub fn load_project(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    watcher: State<ProjectWatcherState>,
    path: String,
) -> Result<(), FrontendError> {
    log_app!(
        LogLevel::Info,
        "[command.load_project] Loading project from: {}",
        path
    );

    let path = normalize_existing_path(&path).map_err(|message| FrontendError {
        code: "INVALID_PATH".into(),
        message,
    })?;

    let project_data = load_project_from_file(&path)?;

    log_app!(
        LogLevel::Info,
        "[command.load_project] Project loaded: {}",
        project_data.info()
    );

    state.activate_loaded_snapshot(&source_store, path.clone(), project_data.clone());
    start_project_watcher(&app, &watcher, &path);
    emit_project_loaded(&app, project_data, path);
    Ok(())
}

/// 将当前项目另存为新目录（完整复制 events/functions/database 等）。
#[tauri::command]
pub async fn save_project_as(
    app: AppHandle,
    state: State<'_, ProjectState>,
    source_store: State<'_, ResultSourceStore>,
    watcher: State<'_, ProjectWatcherState>,
    registry: State<'_, ProjectRegistry>,
    path: String,
) -> Result<ProjectRecord, AppError> {
    log_app!(
        LogLevel::Info,
        "[command.save_project_as] Saving project copy to: {}",
        path
    );

    let new_metadata_path =
        save_project_as_to_directory(&state, &path).map_err(|e| e.to_string())?;

    let project_data = load_project_from_file(&new_metadata_path).map_err(|e| e.to_string())?;

    state.activate_loaded_snapshot(
        &source_store,
        new_metadata_path.clone(),
        project_data.clone(),
    );
    start_project_watcher(&app, &watcher, &new_metadata_path);

    let record = registry
        .register_project(&project_data.metadata.project_name, &new_metadata_path)
        .await?;

    emit_project_loaded(&app, project_data, new_metadata_path);
    Ok(record)
}

#[tauri::command]
pub async fn create_project(
    registry: State<'_, ProjectRegistry>,
    name: String,
    path: String,
) -> Result<ProjectRecord, AppError> {
    let validation = validate_new_project_path_impl(&path);
    if !validation.ok {
        return Err(AppError::new(
            "invalid_project_path",
            validation.message.unwrap_or_else(|| "项目路径无效".into()),
        ));
    }

    let project_root = std::path::PathBuf::from(path.trim());
    std::fs::create_dir_all(&project_root).map_err(|e| format!("无法创建项目文件夹: {e}"))?;
    ensure_project_database_dir(&project_root)
        .map_err(|e| format!("无法创建 database 目录: {e}"))?;
    let project_file_path = project_root.join(PROJECT_METADATA_FILE);

    let mut project_data = ProjectData::new();
    let normalized_name = normalize_project_name(&name);
    project_data.metadata.project_name = normalized_name.clone();
    project_data.update_metadata();
    save_project_to_file(&project_data, project_root.to_string_lossy().as_ref())
        .map_err(|e| e.to_string())?;

    registry
        .register_project(
            &normalized_name,
            project_file_path.to_string_lossy().as_ref(),
        )
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub fn flush_project(app: AppHandle, state: State<ProjectState>) -> Result<(), AppError> {
    let path = state.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
    log_app!(LogLevel::Info, "[command.flush_project] Flushing project");

    state.persist_current_project()?;

    emit_project_event(&app, Event::Project(EventProject::ProjectSaved { path }));
    Ok(())
}

/// 新建项目（清空当前状态）
#[tauri::command]
pub fn new_project(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
) -> Result<(), AppError> {
    log_app!(LogLevel::Info, "[command.new_project] Creating new project");

    state.clear();
    source_store.clear_all();
    emit_project_event(&app, Event::Project(EventProject::ProjectCleared));
    Ok(())
}
