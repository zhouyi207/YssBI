use crate::file_resources::{FileApplicationError, FileMutation};
use crate::ipc::error::CommandError;
use serde::Serialize;
use tauri::AppHandle;
use yss_ipc_contract::{
    event::{Event, EventProject},
    project::ResourceMutationResultDto,
};
use yss_project::file_resources::FileSnapshot;
use yss_project_model::file::FileContent;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCommandResultDto<T: FileContent> {
    snapshot: Option<FileSnapshot<T>>,
    mutation: ResourceMutationResultDto,
}
pub fn command_error(error: FileApplicationError) -> CommandError {
    match error {
        FileApplicationError::Project(error) => {
            super::project_failure::application_project_command_error(error)
        }
        error => CommandError::diagnosed("file_session_changed", error),
    }
}
pub fn publish<T: FileContent>(
    app: &AppHandle,
    result: FileMutation<T>,
) -> Result<FileCommandResultDto<T>, CommandError> {
    let mutation =
        crate::ipc::schema::application_event::resource_mutation_to_transport(&result.mutation);
    yss_ipc_event::emit_project_event_result(
        app,
        &Event::Project(Box::new(EventProject::ResourceMutationCommitted {
            result: mutation.clone(),
        })),
    )
    .map_err(|error| CommandError::diagnosed("file_event_emit_failed", error))?;
    Ok(FileCommandResultDto {
        snapshot: result.snapshot,
        mutation,
    })
}
