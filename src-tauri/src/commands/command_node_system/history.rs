#[cfg(all(test, any()))]
use super::common::mutation_conflict_to_command_error;
#[cfg(all(test, any()))]
use super::common::{EmitOutcome, emit_resource_result};
use crate::error::CommandError;
#[cfg(all(test, any()))]
use crate::event::emit_project_event;
use crate::event::{Event, emit_project_event_result};
use crate::schema::application_event::ResourceMutationResultDto;
use tauri::{AppHandle, State};
#[cfg(all(test, any()))]
use yss_project::ProjectState;
use yss_project_history::{HistoryMutation, HistoryStatusDto, MutationRequest};
use yss_project_identity::ProjectInstanceId;

#[cfg(all(test, any()))]
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
    application: State<'_, yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
) -> Result<HistoryStatusDto, CommandError> {
    application
        .query_history_status(project_instance_id)
        .map(|status| HistoryStatusDto {
            can_undo: status.can_undo,
            can_redo: status.can_redo,
        })
        .map_err(map_history_error)
}

#[cfg(all(test, any()))]
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
        .map_err(mutation_conflict_to_command_error)
}

#[tauri::command]
pub fn undo_graph_document(
    app: AppHandle,
    application: State<'_, yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    locale: String,
    request: MutationRequest<HistoryMutation>,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .undo_graph_document(project_instance_id, locale, request)
        .map_err(map_history_error)?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_history_result(&app, &result)?;
    Ok(result)
}

#[cfg(all(test, any()))]
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
        .map_err(mutation_conflict_to_command_error)
}

#[tauri::command]
pub fn redo_graph_document(
    app: AppHandle,
    application: State<'_, yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    locale: String,
    request: MutationRequest<HistoryMutation>,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .redo_graph_document(project_instance_id, locale, request)
        .map_err(map_history_error)?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
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
    error: yss_application::resource_mutation::ResourceMutationApplicationError,
) -> CommandError {
    super::common::resource_mutation_to_command_error(error, "history_revision_conflict")
}
