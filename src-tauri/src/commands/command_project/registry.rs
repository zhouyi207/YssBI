use crate::error::CommandError;
use crate::event::LifecycleMutationResultDto;
use crate::node_system::document::OperationId;
use crate::project::{
    CleanupInvalidProjectsResult, ProjectInstanceId, ProjectPickerTaskCancelRegistry,
    ProjectRecord, ProjectRegistry, ScanProjectsResult,
};
use tauri::{State, ipc::Channel};

#[tauri::command]
pub async fn list_registered_projects(
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<ProjectRecord>, CommandError> {
    registry
        .list_projects()
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub async fn scan_projects_in_directory(
    registry: State<'_, ProjectRegistry>,
    task_cancel: State<'_, ProjectPickerTaskCancelRegistry>,
    directory: String,
    on_progress: Channel<crate::project::ProjectScanProgressEvent>,
) -> Result<ScanProjectsResult, CommandError> {
    let cancel = task_cancel.begin();
    let result = registry
        .scan_directory(&directory, Some(on_progress), cancel.clone())
        .await;
    task_cancel.end(&cancel);
    result.map_err(CommandError::internal)
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
) -> Result<CleanupInvalidProjectsResult, CommandError> {
    let cancel = task_cancel.begin();
    let result = registry
        .cleanup_invalid_projects(Some(on_progress), cancel.clone())
        .await;
    task_cancel.end(&cancel);
    result.map_err(CommandError::internal)
}

#[tauri::command]
pub async fn register_project(
    registry: State<'_, ProjectRegistry>,
    name: String,
    path: String,
) -> Result<ProjectRecord, CommandError> {
    registry
        .register_project(&name, &path)
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub async fn remove_registered_project(
    registry: State<'_, ProjectRegistry>,
    id: String,
) -> Result<(), CommandError> {
    registry
        .remove_project(&id)
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub async fn delete_registered_project_files(
    app: tauri::AppHandle,
    state: State<'_, crate::project::ProjectState>,
    registry: State<'_, ProjectRegistry>,
    id: String,
    expected_active_instance_id: Option<ProjectInstanceId>,
    operation_id: OperationId,
) -> Result<LifecycleMutationResultDto, CommandError> {
    let result = crate::application::project_lifecycle::delete_registered_project(
        state.inner(),
        registry.inner(),
        &id,
        expected_active_instance_id,
        operation_id,
    )
    .await
    .map_err(super::lifecycle::map_project_lifecycle_error)?;
    super::lifecycle::publish_lifecycle_result(&app, &result);
    Ok(result)
}

#[tauri::command]
pub async fn toggle_registered_project_favorite(
    registry: State<'_, ProjectRegistry>,
    id: String,
) -> Result<bool, CommandError> {
    registry
        .toggle_favorite(&id)
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub fn get_project_registry_path(registry: State<ProjectRegistry>) -> String {
    registry.path().to_string_lossy().into_owned()
}
