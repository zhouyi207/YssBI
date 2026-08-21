use super::common::{EmitOutcome, emit_resource_result, mutation_conflict_to_command_error};
use crate::error::CommandError;
use crate::event::{Event, ResourceMutationResultDto, emit_project_event};
use crate::node_system::document::{HistoryMutation, HistoryStatusDto, MutationRequest};
use crate::project::{ProjectInstanceId, ProjectState};
use tauri::{AppHandle, State};

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
) -> Result<HistoryStatusDto, CommandError> {
    get_project_history_status_from_state(state.inner(), project_instance_id)
}

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    locale: String,
    request: MutationRequest<HistoryMutation>,
) -> Result<ResourceMutationResultDto, CommandError> {
    undo_graph_document_with_emitter(
        state.inner(),
        project_instance_id,
        &locale,
        request,
        |event| emit_project_event(&app, event),
    )
}

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    locale: String,
    request: MutationRequest<HistoryMutation>,
) -> Result<ResourceMutationResultDto, CommandError> {
    redo_graph_document_with_emitter(
        state.inner(),
        project_instance_id,
        &locale,
        request,
        |event| emit_project_event(&app, event),
    )
}
