use super::common::mutation_conflict_to_command_error;
#[cfg(test)]
use super::common::{EmitOutcome, emit_resource_result};
use crate::error::CommandError;
#[cfg(test)]
use crate::event::emit_project_event;
use crate::event::{Event, ResourceMutationResultDto, emit_project_event_result};
use crate::node_system::document::{HistoryMutation, HistoryStatusDto, MutationRequest};
use crate::project::ProjectInstanceId;
#[cfg(test)]
use crate::project::ProjectState;
use tauri::{AppHandle, State};

#[cfg(test)]
pub(super) fn get_project_history_status_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
) -> Result<HistoryStatusDto, CommandError> {
    state
        .history_status_for_project(&project_instance_id)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn get_project_history_status(
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
) -> Result<HistoryStatusDto, CommandError> {
    application
        .query_history_status(project_instance_id)
        .map_err(map_history_error)
}

#[cfg(test)]
pub(super) fn undo_graph_document_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    locale: &str,
    request: MutationRequest<HistoryMutation>,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, CommandError> {
    state
        .undo_last_transaction_observed(&project_instance_id, locale, request, |result| {
            emit_resource_result(&mut emit, result)
        })
        .map_err(|error| mutation_conflict_to_command_error(error, "history_revision_conflict"))
}

#[tauri::command]
pub fn undo_graph_document(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    locale: String,
    request: MutationRequest<HistoryMutation>,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .undo_graph_document(project_instance_id, locale, request)
        .map_err(map_history_error)?;
    emit_application_history_result(&app, &result)?;
    Ok(result)
}

#[cfg(test)]
pub(super) fn redo_graph_document_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    locale: &str,
    request: MutationRequest<HistoryMutation>,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, CommandError> {
    state
        .redo_last_transaction_observed(&project_instance_id, locale, request, |result| {
            emit_resource_result(&mut emit, result)
        })
        .map_err(|error| mutation_conflict_to_command_error(error, "history_revision_conflict"))
}

#[tauri::command]
pub fn redo_graph_document(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    locale: String,
    request: MutationRequest<HistoryMutation>,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .redo_graph_document(project_instance_id, locale, request)
        .map_err(map_history_error)?;
    emit_application_history_result(&app, &result)?;
    Ok(result)
}

fn emit_application_history_result(
    app: &AppHandle,
    result: &ResourceMutationResultDto,
) -> Result<(), CommandError> {
    emit_project_event_result(
        app,
        &Event::Project(crate::event::EventProject::ResourceMutationCommitted {
            result: result.clone(),
        }),
    )
    .map_err(|error| CommandError::diagnosed("history_event_emit_failed", error))
}

fn map_history_error(
    error: crate::application::resource_mutation::ResourceMutationApplicationError,
) -> CommandError {
    use crate::application::resource_mutation::ResourceMutationApplicationError;
    match error {
        ResourceMutationApplicationError::SessionCapture(error) => match error {
            crate::application::execution::SessionCaptureError::Inactive => {
                CommandError::expected("stale_project_lifecycle")
            }
            crate::application::execution::SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            crate::application::execution::SessionCaptureError::Recovering => {
                CommandError::expected("project_recovery_required")
                    .with_details(serde_json::json!({ "recoveryRequired": true }))
            }
        },
        ResourceMutationApplicationError::Project(error) => CommandError::from(error),
        ResourceMutationApplicationError::Mutation(error) => {
            mutation_conflict_to_command_error(error, "history_revision_conflict")
        }
        ResourceMutationApplicationError::SessionChanged(error) => {
            CommandError::diagnosed("history_session_changed", error)
        }
        ResourceMutationApplicationError::SessionRefresh(error) => {
            CommandError::diagnosed("history_session_refresh_failed", error)
        }
    }
}
