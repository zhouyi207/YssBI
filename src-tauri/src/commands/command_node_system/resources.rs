use super::common::parse_graph_path;
#[cfg(test)]
use super::common::{EmitOutcome, emit_resource_result, mutation_conflict_to_command_error};
use crate::error::CommandError;
#[cfg(test)]
use crate::event::emit_project_event;
use crate::event::{Event, EventProject, ResourceMutationResultDto, emit_project_event_result};
#[cfg(test)]
use crate::graph_document::GraphResourcePath;
use crate::node_system::document::MutationRequest;
use crate::project::ProjectInstanceId;
#[cfg(test)]
use crate::project::ProjectState;
use crate::project::project_writers::ProjectSaveResultDto;
use crate::project::{OperationId, ResourceRevision};
use tauri::{AppHandle, State};

#[cfg(test)]
pub(super) fn create_graph_resource_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_name: &str,
    kind: crate::project::GraphDocumentKind,
    operation_id: OperationId,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = state.create_graph_resource_transaction(
        &project_instance_id,
        graph_name,
        kind,
        operation_id,
    )?;
    emit_resource_result(&mut emit, &result);
    Ok(result)
}

#[tauri::command]
pub fn create_event(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_name: String,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .create_graph_resource(
            project_instance_id,
            graph_name,
            crate::project::GraphDocumentKind::Event,
            operation_id,
        )
        .map_err(map_resource_mutation_error)?;
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn create_function(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_name: String,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .create_graph_resource(
            project_instance_id,
            graph_name,
            crate::project::GraphDocumentKind::Function,
            operation_id,
        )
        .map_err(map_resource_mutation_error)?;
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn unload_project_graph(
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: String,
    graph_path: String,
    lifecycle_token: u64,
) -> Result<(), CommandError> {
    let project_instance_id = crate::project::ProjectInstanceId::from_existing(project_instance_id);
    application
        .unload_graph_resource(
            project_instance_id,
            parse_graph_path(graph_path)?,
            lifecycle_token,
        )
        .map_err(map_resource_mutation_error)?;
    Ok(())
}

#[cfg(test)]
pub(super) fn save_project_graph_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mut emit: impl FnMut(Event),
) -> Result<ProjectSaveResultDto, CommandError> {
    let result = state
        .save_graph_document(
            &project_instance_id,
            &graph_path,
            expected_revision,
            operation_id,
        )
        .map_err(CommandError::from)?;
    emit(Event::Project(EventProject::ProjectSaved {
        result: result.clone(),
    }));
    Ok(result)
}

#[tauri::command]
pub fn save_project_graph(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ProjectSaveResultDto, CommandError> {
    let result = application
        .save_graph_resource(
            project_instance_id,
            parse_graph_path(graph_path)?,
            expected_revision,
            operation_id,
        )
        .map_err(map_resource_mutation_error)?;
    emit_project_event_result(
        &app,
        &Event::Project(EventProject::ProjectSaved {
            result: result.clone(),
        }),
    )
    .map_err(|error| CommandError::diagnosed("project_event_emit_failed", error))?;
    Ok(result)
}

#[cfg(test)]
pub(super) fn duplicate_graph_resource_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = state.duplicate_graph_resource_transaction(
        &project_instance_id,
        &graph_path,
        expected_revision,
        operation_id,
    )?;
    emit_resource_result(&mut emit, &result);
    Ok(result)
}

#[tauri::command]
pub fn duplicate_graph(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .duplicate_graph_resource(
            project_instance_id,
            parse_graph_path(graph_path)?,
            expected_revision,
            operation_id,
        )
        .map_err(map_resource_mutation_error)?;
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[cfg(test)]
pub(super) fn remove_graph_resource_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = state.remove_graph_resource_transaction(
        &project_instance_id,
        &graph_path,
        expected_revision,
        operation_id,
    )?;
    emit_resource_result(&mut emit, &result);
    Ok(result)
}

#[tauri::command]
pub fn remove_graph(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .remove_graph_resource(
            project_instance_id,
            parse_graph_path(graph_path)?,
            expected_revision,
            operation_id,
        )
        .map_err(map_resource_mutation_error)?;
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[cfg(test)]
pub(super) fn rename_graph_resource_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    expected_revision: ResourceRevision,
    new_name: &str,
    lifecycle_token: u64,
    operation_id: OperationId,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = state.rename_graph_resource_transaction(
        &project_instance_id,
        &graph_path,
        expected_revision,
        new_name,
        lifecycle_token,
        operation_id,
    )?;
    emit_resource_result(&mut emit, &result);
    Ok(result)
}

#[tauri::command]
pub fn rename_graph_resource(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    new_name: String,
    lifecycle_token: u64,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .rename_graph_resource(
            project_instance_id,
            parse_graph_path(graph_path)?,
            expected_revision,
            new_name,
            lifecycle_token,
            operation_id,
        )
        .map_err(map_resource_mutation_error)?;
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[cfg(test)]
pub(super) fn update_function_signature_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    function_path: String,
    locale: &str,
    request: MutationRequest<crate::node_system::document::FunctionDocumentPatch>,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, CommandError> {
    let path = parse_graph_path(function_path)?;
    state
        .update_function_signature_observed(
            &project_instance_id,
            &path,
            locale,
            request,
            |result| emit_resource_result(&mut emit, result),
        )
        .map_err(|error| mutation_conflict_to_command_error(error, "function_revision_conflict"))
}

#[tauri::command]
pub fn update_function_signature(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    function_path: String,
    locale: String,
    request: MutationRequest<crate::node_system::document::FunctionDocumentPatch>,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .update_function_signature(
            project_instance_id,
            parse_graph_path(function_path)?,
            locale,
            request,
        )
        .map_err(map_resource_mutation_error)?;
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

fn emit_application_resource_result(
    app: &AppHandle,
    result: &ResourceMutationResultDto,
) -> Result<(), CommandError> {
    emit_project_event_result(
        app,
        &Event::Project(EventProject::ResourceMutationCommitted {
            result: result.clone(),
        }),
    )
    .map_err(|error| CommandError::diagnosed("resource_event_emit_failed", error))
}

fn map_resource_mutation_error(
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
            super::common::mutation_conflict_to_command_error(error, "graph_revision_conflict")
        }
        ResourceMutationApplicationError::SessionChanged(error) => {
            CommandError::diagnosed("resource_session_changed", error)
        }
        ResourceMutationApplicationError::SessionRefresh(error) => {
            CommandError::diagnosed("resource_session_refresh_failed", error)
        }
    }
}
