use crate::ipc::error::CommandError;
use crate::project_lifecycle::ProjectManagement;
use std::time::{Duration, Instant};
use tauri::{State, ipc::Channel};
use yss_ipc_channel::project_progress::ProgressAdapterShutdownControl;
use yss_ipc_channel::project_progress::ProjectProgressAdapterSpawnError;
use yss_ipc_channel::project_progress::ProjectProgressDrainOutcome;
use yss_ipc_channel::project_progress::bounded_project_progress_adapter;
use yss_ipc_channel::project_progress::reap_project_progress_worker;
use yss_ipc_contract::project::LifecycleMutationResultDto;
use yss_ipc_contract::project_progress::ProjectProgressDto;
use yss_project_identity::OperationId;
use yss_project_identity::ProjectInstanceId;
use yss_project_registry::{CleanupInvalidProjectsResult, ScanProjectsResult};
use yss_project_registry_contract::ProjectRecord;

fn project_progress_adapter_spawn_error(error: ProjectProgressAdapterSpawnError) -> CommandError {
    CommandError::diagnosed("project_progress_worker_spawn_failed", error)
}

#[tauri::command]
pub async fn list_registered_projects(
    projects: State<'_, ProjectManagement>,
) -> Result<Vec<ProjectRecord>, CommandError> {
    projects
        .list_projects()
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub async fn scan_projects_in_directory(
    projects: State<'_, ProjectManagement>,
    directory: String,
    on_progress: Channel<ProjectProgressDto>,
) -> Result<ScanProjectsResult, CommandError> {
    let (publisher, worker) = bounded_project_progress_adapter(on_progress)
        .map_err(project_progress_adapter_spawn_error)?;
    let result = projects
        .scan_directory(&directory, publisher.as_ref())
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
    result.map_err(CommandError::internal)
}

#[tauri::command]
pub fn cancel_project_picker_task(projects: State<'_, ProjectManagement>) {
    projects.cancel_picker_task();
}

#[tauri::command]
pub async fn cleanup_invalid_registered_projects(
    projects: State<'_, ProjectManagement>,
    on_progress: Channel<ProjectProgressDto>,
) -> Result<CleanupInvalidProjectsResult, CommandError> {
    let (publisher, worker) = bounded_project_progress_adapter(on_progress)
        .map_err(project_progress_adapter_spawn_error)?;
    let result = projects.cleanup_invalid_projects(publisher.as_ref()).await;
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
    result.map_err(CommandError::internal)
}

#[tauri::command]
pub async fn register_project(
    projects: State<'_, ProjectManagement>,
    name: String,
    path: String,
) -> Result<ProjectRecord, CommandError> {
    projects
        .register_project(&name, &path)
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub async fn remove_registered_project(
    projects: State<'_, ProjectManagement>,
    id: String,
) -> Result<(), CommandError> {
    projects
        .remove_project(&id)
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub async fn delete_registered_project_files(
    app: tauri::AppHandle,
    application: State<'_, crate::execution::ApplicationState>,
    projects: State<'_, ProjectManagement>,
    id: String,
    expected_active_instance_id: Option<ProjectInstanceId>,
    operation_id: OperationId,
) -> Result<LifecycleMutationResultDto, CommandError> {
    let result = application
        .delete_registered_project_for_application(
            projects.inner(),
            &id,
            expected_active_instance_id,
            operation_id,
        )
        .await
        .map_err(super::lifecycle::map_application_project_lifecycle_error)?;
    let result = crate::ipc::schema::application_event::project_lifecycle_to_transport(&result);
    super::lifecycle::publish_lifecycle_result(&app, &result);
    Ok(result)
}

#[tauri::command]
pub async fn toggle_registered_project_favorite(
    projects: State<'_, ProjectManagement>,
    id: String,
) -> Result<bool, CommandError> {
    projects
        .toggle_favorite(&id)
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub fn get_project_registry_path(projects: State<ProjectManagement>) -> String {
    projects.path().to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    #[test]
    fn progress_worker_spawn_failure_maps_to_stable_diagnosed_error() {
        let error = ProjectProgressAdapterSpawnError::from(io::Error::other("injected failure"));

        let command_error = project_progress_adapter_spawn_error(error);

        assert_eq!(command_error.code(), "project_progress_worker_spawn_failed");
        assert!(command_error.incident_id().is_some());
    }
}
