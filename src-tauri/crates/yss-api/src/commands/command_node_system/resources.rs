use super::common::parse_graph_path;
use crate::error::CommandError;
use crate::event::{Event, EventProject, emit_project_event_result};
use crate::schema::application_event::ResourceMutationResultDto;
use crate::schema::graph_draft::GraphDraftSaveDto;
use tauri::{AppHandle, State};
use yss_project_history::MutationRequest;
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};

#[tauri::command]
pub fn create_event(
    app: AppHandle,
    application: State<'_, yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_name: String,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .create_graph_resource(
            project_instance_id,
            graph_name,
            yss_graph_document::GraphResourceKind::Event,
            operation_id,
        )
        .map_err(map_resource_mutation_error)?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn create_function(
    app: AppHandle,
    application: State<'_, yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_name: String,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .create_graph_resource(
            project_instance_id,
            graph_name,
            yss_graph_document::GraphResourceKind::Function,
            operation_id,
        )
        .map_err(map_resource_mutation_error)?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn unload_project_graph(
    application: State<'_, yss_application::execution::ApplicationState>,
    project_instance_id: String,
    graph_path: String,
    lifecycle_token: u64,
) -> Result<(), CommandError> {
    let project_instance_id =
        yss_project_identity::ProjectInstanceId::from_existing(project_instance_id);
    application
        .unload_graph_resource(
            project_instance_id,
            parse_graph_path(graph_path)?,
            lifecycle_token,
        )
        .map_err(map_resource_mutation_error)?;
    Ok(())
}

#[tauri::command]
pub fn save_project_graph(
    application: State<'_, yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
    document: yss_graph_document::GraphDocument,
    operation_id: OperationId,
) -> Result<GraphDraftSaveDto, CommandError> {
    let result = application
        .save_graph_draft(
            project_instance_id,
            parse_graph_path(graph_path)?,
            locale,
            operation_id,
            document,
        )
        .map_err(map_graph_draft_save_error)?;
    Ok(crate::schema::graph_draft::graph_draft_save_to_transport(
        &result,
    ))
}

#[tauri::command]
pub fn duplicate_graph(
    app: AppHandle,
    application: State<'_, yss_application::execution::ApplicationState>,
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
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn remove_graph(
    app: AppHandle,
    application: State<'_, yss_application::execution::ApplicationState>,
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
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn rename_graph_resource(
    app: AppHandle,
    application: State<'_, yss_application::execution::ApplicationState>,
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
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn update_function_signature(
    app: AppHandle,
    application: State<'_, yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    function_path: String,
    locale: String,
    request: MutationRequest<yss_project_history::FunctionDocumentPatch>,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .update_function_signature(
            project_instance_id,
            parse_graph_path(function_path)?,
            locale,
            request,
        )
        .map_err(map_resource_mutation_error)?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
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
    error: yss_application::resource_mutation::ResourceMutationApplicationError,
) -> CommandError {
    super::common::resource_mutation_to_command_error(error, "graph_revision_conflict")
}

fn map_graph_draft_save_error(
    error: yss_application::resource_mutation::ResourceMutationApplicationError,
) -> CommandError {
    super::common::resource_mutation_to_command_error(error, "graph_save_rejected")
}
