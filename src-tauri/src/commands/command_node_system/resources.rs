use super::common::{
    EmitOutcome, emit_resource_result, mutation_conflict_to_command_error, parse_graph_path,
};
use crate::error::CommandError;
use crate::event::{Event, EventProject, ResourceMutationResultDto, emit_project_event};
use crate::graph_document::GraphResourcePath;
use crate::node_system::document::MutationRequest;
use crate::project::project_writers::ProjectSaveResultDto;
use crate::project::{OperationId, ResourceRevision};
use crate::project::{ProjectInstanceId, ProjectState};
use tauri::{AppHandle, State};

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_name: String,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    create_graph_resource_with_emitter(
        state.inner(),
        project_instance_id,
        &graph_name,
        crate::project::GraphDocumentKind::Event,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn create_function(
    app: AppHandle,
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_name: String,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    create_graph_resource_with_emitter(
        state.inner(),
        project_instance_id,
        &graph_name,
        crate::project::GraphDocumentKind::Function,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn unload_project_graph(
    state: State<'_, ProjectState>,
    project_instance_id: String,
    graph_path: String,
    lifecycle_token: u64,
) -> Result<(), CommandError> {
    let project_instance_id = crate::project::ProjectInstanceId::from_existing(project_instance_id);
    state.unload_graph_resource_for_lifecycle(
        &project_instance_id,
        &parse_graph_path(graph_path)?,
        lifecycle_token,
    )?;
    Ok(())
}

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ProjectSaveResultDto, CommandError> {
    save_project_graph_with_emitter(
        state.inner(),
        project_instance_id,
        parse_graph_path(graph_path)?,
        expected_revision,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    duplicate_graph_resource_with_emitter(
        state.inner(),
        project_instance_id,
        parse_graph_path(graph_path)?,
        expected_revision,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    remove_graph_resource_with_emitter(
        state.inner(),
        project_instance_id,
        parse_graph_path(graph_path)?,
        expected_revision,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    new_name: String,
    lifecycle_token: u64,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    rename_graph_resource_with_emitter(
        state.inner(),
        project_instance_id,
        parse_graph_path(graph_path)?,
        expected_revision,
        &new_name,
        lifecycle_token,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    function_path: String,
    locale: String,
    request: MutationRequest<crate::node_system::document::FunctionDocumentPatch>,
) -> Result<ResourceMutationResultDto, CommandError> {
    update_function_signature_with_emitter(
        state.inner(),
        project_instance_id,
        function_path,
        &locale,
        request,
        |event| emit_project_event(&app, event),
    )
}
