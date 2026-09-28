use super::file_resource::{FileCommandResultDto, command_error, publish};
use crate::{ipc::error::CommandError, session::ApplicationState};
use tauri::{AppHandle, State};
use yss_project::minds::{MindCommand, MindSnapshot};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_model::mind::{MindDocument, MindPath};
#[tauri::command]
pub fn read_project_mind(
    application: State<ApplicationState>,
    project_instance_id: ProjectInstanceId,
    path: MindPath,
) -> Result<MindSnapshot, CommandError> {
    application
        .read_mind(project_instance_id, path)
        .map_err(command_error)
}
#[tauri::command]
pub fn edit_project_mind(
    app: AppHandle,
    application: State<ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    command: MindCommand,
) -> Result<FileCommandResultDto<MindDocument>, CommandError> {
    let result = application
        .apply_mind_command(project_instance_id, operation_id, command)
        .map_err(command_error)?;
    publish(&app, result)
}
