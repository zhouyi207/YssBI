use crate::error::AppError;
use crate::event::{Event, EventProject, emit_project_event};

use crate::frontend::FrontendError;
use crate::log::LogLevel;
use crate::log_app;
use crate::node_system::document::OperationId;
use crate::project::project_writers::ProjectSaveResultDto;
use crate::project::{
    NormalizedProjectRoot, PROJECT_METADATA_FILE, ProjectData, ProjectInstanceId, ProjectRecord,
    ProjectRegistry, ProjectState, ProjectWatcherState, ensure_project_database_dir,
    normalize_existing_path, normalize_project_name, save_project_as_to_directory,
    validate_new_project_path as validate_new_project_path_impl,
};
use tauri::{AppHandle, State};

fn emit_project_loaded(app: &AppHandle, path: String) {
    emit_project_event(
        app,
        Event::Project(EventProject::ProjectLoaded { path: Some(path) }),
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

    state
        .activate_project_from_path(std::path::Path::new(&path))
        .map_err(|error| FrontendError {
            code: "LOAD_PROJECT_FAILED".into(),
            message: error.to_string(),
        })?;
    let project_data = state.get_data().map_err(|error| FrontendError {
        code: "LOAD_PROJECT_FAILED".into(),
        message: error.to_string(),
    })?;

    log_app!(
        LogLevel::Info,
        "[command.load_project] Project loaded: {}",
        project_data.info()
    );

    start_project_watcher(&app, &watcher, &path);
    emit_project_loaded(&app, path);
    Ok(())
}

/// 将当前项目另存为新目录（完整复制 events/functions/database 等）。
#[tauri::command]
pub async fn save_project_as(
    app: AppHandle,
    state: State<'_, ProjectState>,
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

    state.activate_project_from_path(std::path::Path::new(&new_metadata_path))?;
    let project_data = state.get_data()?;
    start_project_watcher(&app, &watcher, &new_metadata_path);

    let record = registry
        .register_project(&project_data.metadata.project_name, &new_metadata_path)
        .await?;

    emit_project_loaded(&app, new_metadata_path);
    Ok(record)
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, ProjectState>,
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
    let mut project_data = ProjectData::new();
    let normalized_name = normalize_project_name(&name);
    project_data.metadata.project_name = normalized_name.clone();
    project_data.update_metadata();
    let normalized_root =
        NormalizedProjectRoot::from_project_path(&path).map_err(|error| error.to_string())?;
    let _filesystem_lease = state
        .filesystem()
        .acquire(normalized_root)
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&project_root).map_err(|e| format!("无法创建项目文件夹: {e}"))?;
    ensure_project_database_dir(&project_root)
        .map_err(|e| format!("无法创建 database 目录: {e}"))?;
    crate::project::initialize_project_directory(&project_data, &project_root)
        .map_err(|e| e.to_string())?;
    let project_file_path = project_root.join(PROJECT_METADATA_FILE);

    registry
        .register_project(
            &normalized_name,
            project_file_path.to_string_lossy().as_ref(),
        )
        .await
        .map_err(AppError::from)
}

pub(crate) fn flush_project_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    mut emit: impl FnMut(Event),
) -> Result<ProjectSaveResultDto, AppError> {
    let result = state
        .flush_project_documents(&project_instance_id, operation_id)
        .map_err(AppError::from)?;
    emit(Event::Project(EventProject::ProjectSaved {
        result: result.clone(),
    }));
    Ok(result)
}

#[tauri::command]
pub fn flush_project(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
) -> Result<ProjectSaveResultDto, AppError> {
    log_app!(LogLevel::Info, "[command.flush_project] Flushing project");
    flush_project_with_emitter(state.inner(), project_instance_id, operation_id, |event| {
        emit_project_event(&app, event)
    })
}

/// 新建项目（清空当前状态）
#[tauri::command]
pub fn new_project(app: AppHandle, state: State<ProjectState>) -> Result<(), AppError> {
    log_app!(LogLevel::Info, "[command.new_project] Creating new project");

    state.clear_project()?;
    emit_project_event(&app, Event::Project(EventProject::ProjectCleared));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::document::OperationId;

    #[test]
    fn flush_command_returns_correlated_result_emits_once_and_stale_emits_nothing() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-flush-command-contract-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let operation_id = OperationId::new();
        let mut events = Vec::new();

        let result = flush_project_with_emitter(
            &state,
            project_instance_id.clone(),
            operation_id,
            |event| events.push(event),
        )
        .unwrap();

        assert_eq!(result.project_instance_id, project_instance_id.as_str());
        assert_eq!(result.operation_id, operation_id);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            Event::Project(EventProject::ProjectSaved { result: emitted }) if emitted == &result
        ));

        state.activate_project_fixture(
            root.to_string_lossy().into_owned(),
            state.get_data().unwrap(),
        );
        let error =
            flush_project_with_emitter(&state, project_instance_id, OperationId::new(), |event| {
                events.push(event)
            })
            .unwrap_err();
        assert_eq!(error.code, "stale_project_lifecycle");
        assert_eq!(events.len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }
}
