use super::file_resource::{FileCommandResultDto, command_error, publish};
use crate::{ipc::error::CommandError, session::ApplicationState};
use tauri::{AppHandle, State};
use yss_project::docs::{DocCommand, DocSnapshot};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_model::doc::{DocDocument, DocPath};
#[tauri::command]
pub fn read_project_doc(
    application: State<ApplicationState>,
    project_instance_id: ProjectInstanceId,
    path: DocPath,
) -> Result<DocSnapshot, CommandError> {
    application
        .read_doc(project_instance_id, path)
        .map_err(command_error)
}
#[tauri::command]
pub fn edit_project_doc(
    app: AppHandle,
    application: State<ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    command: DocCommand,
) -> Result<FileCommandResultDto<DocDocument>, CommandError> {
    let result = application
        .apply_doc_command(project_instance_id, operation_id, command)
        .map_err(command_error)?;
    publish(&app, result)
}
