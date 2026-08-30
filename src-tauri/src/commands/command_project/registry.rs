use super::progress::{
    ProgressAdapterShutdownControl, ProjectProgressDrainOutcome, ProjectProgressDto,
    bounded_project_progress_adapter, reap_project_progress_worker,
};
use crate::error::CommandError;
use crate::project::{CleanupInvalidProjectsResult, ProjectRegistry, ScanProjectsResult};
use crate::schema::application_event::LifecycleMutationResultDto;
use std::time::{Duration, Instant};
use tauri::{State, ipc::Channel};
use yss_project_identity::OperationId;
use yss_project_identity::ProjectInstanceId;
use yss_project_progress::ProjectTaskCancellationRegistry;
use yss_project_registry_contract::ProjectRecord;

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
    task_cancel: State<'_, ProjectTaskCancellationRegistry>,
    directory: String,
    on_progress: Channel<ProjectProgressDto>,
) -> Result<ScanProjectsResult, CommandError> {
    let cancel = task_cancel.begin();
    let (publisher, worker) = bounded_project_progress_adapter(on_progress);
    let result = registry
        .scan_directory(&directory, Some(publisher.as_ref()), cancel.clone())
        .await;
    publisher.close();
    drop(publisher);
    match worker.finish(ProgressAdapterShutdownControl::new(
        Instant::now() + Duration::from_secs(1),
    )) {
        ProjectProgressDrainOutcome::TimedOut(worker) => reap_project_progress_worker(worker),
        ProjectProgressDrainOutcome::Drained(Ok(())) => {}
        ProjectProgressDrainOutcome::Drained(Err(error)) => {
            tracing::warn!(
                target: "yssbi::commands::project",
                diagnostic_domain = "system",
                error_kind = ?error,
                "Project progress delivery failed"
            );
        }
    }
    task_cancel.end(&cancel);
    result.map_err(CommandError::internal)
}

#[tauri::command]
pub fn cancel_project_picker_task(task_cancel: State<'_, ProjectTaskCancellationRegistry>) {
    task_cancel.cancel_active();
}

#[tauri::command]
pub async fn cleanup_invalid_registered_projects(
    registry: State<'_, ProjectRegistry>,
    task_cancel: State<'_, ProjectTaskCancellationRegistry>,
    on_progress: Channel<ProjectProgressDto>,
) -> Result<CleanupInvalidProjectsResult, CommandError> {
    let cancel = task_cancel.begin();
    let (publisher, worker) = bounded_project_progress_adapter(on_progress);
    let result = registry
        .cleanup_invalid_projects(Some(publisher.as_ref()), cancel.clone())
        .await;
    publisher.close();
    drop(publisher);
    match worker.finish(ProgressAdapterShutdownControl::new(
        Instant::now() + Duration::from_secs(1),
    )) {
        ProjectProgressDrainOutcome::TimedOut(worker) => reap_project_progress_worker(worker),
        ProjectProgressDrainOutcome::Drained(Ok(())) => {}
        ProjectProgressDrainOutcome::Drained(Err(error)) => {
            tracing::warn!(
                target: "yssbi::commands::project",
                diagnostic_domain = "system",
                error_kind = ?error,
                "Project progress delivery failed"
            );
        }
    }
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
    application: State<'_, crate::application::execution::ApplicationState>,
    registry: State<'_, ProjectRegistry>,
    id: String,
    expected_active_instance_id: Option<ProjectInstanceId>,
    operation_id: OperationId,
) -> Result<LifecycleMutationResultDto, CommandError> {
    let result = application
        .delete_registered_project_for_application(
            registry.inner(),
            &id,
            expected_active_instance_id,
            operation_id,
        )
        .await
        .map_err(super::lifecycle::map_application_project_lifecycle_error)?;
    let result = crate::schema::application_event::project_lifecycle_to_transport(&result);
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
