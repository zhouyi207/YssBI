use crate::event::{Event, EventProject, emit_project_event};
use crate::project::{
    CleanupInvalidProjectsResult, ProjectPickerTaskCancelRegistry, ProjectRecord, ProjectRegistry,
    ScanProjectsResult,
};
use tauri::{State, ipc::Channel};

#[tauri::command]
pub async fn list_registered_projects(
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<ProjectRecord>, String> {
    registry.list_projects().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_projects_in_directory(
    registry: State<'_, ProjectRegistry>,
    task_cancel: State<'_, ProjectPickerTaskCancelRegistry>,
    directory: String,
    on_progress: Channel<crate::project::ProjectScanProgressEvent>,
) -> Result<ScanProjectsResult, String> {
    let cancel = task_cancel.begin();
    let result = registry
        .scan_directory(&directory, Some(on_progress), cancel.clone())
        .await;
    task_cancel.end(&cancel);
    result
}

#[tauri::command]
pub fn cancel_project_picker_task(task_cancel: State<'_, ProjectPickerTaskCancelRegistry>) {
    task_cancel.cancel_active();
}

#[tauri::command]
pub async fn cleanup_invalid_registered_projects(
    registry: State<'_, ProjectRegistry>,
    task_cancel: State<'_, ProjectPickerTaskCancelRegistry>,
    on_progress: Channel<crate::project::ProjectCleanupProgressEvent>,
) -> Result<CleanupInvalidProjectsResult, String> {
    let cancel = task_cancel.begin();
    let result = registry
        .cleanup_invalid_projects(Some(on_progress), cancel.clone())
        .await;
    task_cancel.end(&cancel);
    result
}

#[tauri::command]
pub async fn register_project(
    registry: State<'_, ProjectRegistry>,
    name: String,
    path: String,
) -> Result<ProjectRecord, String> {
    registry.register_project(&name, &path).await
}

#[tauri::command]
pub async fn remove_registered_project(
    registry: State<'_, ProjectRegistry>,
    id: String,
) -> Result<(), String> {
    registry.remove_project(&id).await
}

#[tauri::command]
pub async fn delete_registered_project_files(
    app: tauri::AppHandle,
    state: State<'_, crate::project::ProjectState>,
    source_store: State<'_, crate::execution::ResultSourceStore>,
    registry: State<'_, ProjectRegistry>,
    id: String,
) -> Result<(), String> {
    use crate::project::{delete_project_directory, paths_refer_to_same_project};

    let record = registry
        .fetch_by_id(&id)
        .await?
        .ok_or_else(|| "项目不存在".to_string())?;

    let deleting_active = state
        .get_path()
        .is_some_and(|loaded| paths_refer_to_same_project(&loaded, &record.path));

    delete_project_directory(&record.path).map_err(|e| e.to_string())?;
    registry.remove_project(&id).await?;

    if deleting_active {
        state.clear();
        source_store.clear_all();
        emit_project_event(&app, Event::Project(EventProject::ProjectCleared));
    }

    Ok(())
}

#[tauri::command]
pub async fn toggle_registered_project_favorite(
    registry: State<'_, ProjectRegistry>,
    id: String,
) -> Result<bool, String> {
    registry.toggle_favorite(&id).await
}

#[tauri::command]
pub fn get_project_registry_path(registry: State<ProjectRegistry>) -> String {
    registry.path().to_string_lossy().into_owned()
}
