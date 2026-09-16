use super::common::parse_graph_path;
use crate::ipc::error::CommandError;
use crate::ipc::graph_editor_sync::{Binding, GraphEditorSyncState};
use tauri::{AppHandle, State};
use yss_ipc_contract::event::Event;
use yss_ipc_contract::event::EventProject;
use yss_ipc_contract::graph_editing::GraphEditorSyncResponseDto;
use yss_ipc_contract::project::ResourceMutationResultDto;
use yss_ipc_event::emit_project_event_result;
use yss_project_history::MutationRequest;
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};

#[tauri::command]
pub fn create_event(
    app: AppHandle,
    application: State<'_, crate::session::ApplicationState>,
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
    let result = crate::ipc::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn create_function(
    app: AppHandle,
    application: State<'_, crate::session::ApplicationState>,
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
    let result = crate::ipc::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn unload_project_graph(
    application: State<'_, crate::session::ApplicationState>,
    project_instance_id: String,
    graph_path: String,
    lifecycle_token: u64,
    discard_version: Option<yss_ipc_contract::graph_editing::GraphEditVersionDto>,
) -> Result<bool, CommandError> {
    let project_instance_id =
        yss_project_identity::ProjectInstanceId::from_existing(project_instance_id);
    application
        .unload_graph_resource(
            project_instance_id,
            parse_graph_path(graph_path)?,
            lifecycle_token,
            discard_version
                .map(crate::ipc::schema::graph_editing::graph_edit_version_from_transport)
                .transpose()?,
        )
        .map_err(map_resource_mutation_error)
}

#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "Tauri binds graph save identity and transport context"
)]
pub async fn save_project_graph(
    window: tauri::WebviewWindow,
    sync: State<'_, GraphEditorSyncState>,
    cursor: Option<String>,
    application: State<'_, crate::session::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
    version: yss_ipc_contract::graph_editing::GraphEditVersionDto,
    operation_id: OperationId,
) -> Result<GraphEditorSyncResponseDto, CommandError> {
    let binding = Binding {
        window: window.label().into(),
        project: project_instance_id.to_string(),
        graph: graph_path.clone(),
        locale: locale.clone(),
    };
    let application = application.inner().clone();
    let sync = sync.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = application
            .save_current_graph(crate::graph::editing::GraphEditRequest {
                project_instance_id,
                graph_path: parse_graph_path(graph_path)?,
                locale,
                operation_id,
                version: crate::ipc::schema::graph_editing::graph_edit_version_from_transport(
                    version,
                )?,
            })
            .map_err(map_graph_draft_save_error)?;
        let mut response = crate::ipc::schema::graph_editing::encode_graph_edit(
            &sync,
            binding,
            cursor.as_deref(),
            &result.graph,
        )?;
        response.resource_revision = Some(result.resource_revision);
        Ok(response)
    })
    .await
    .map_err(CommandError::internal)?
}

#[tauri::command]
pub fn duplicate_graph(
    app: AppHandle,
    application: State<'_, crate::session::ApplicationState>,
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
    let result = crate::ipc::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn remove_graph(
    app: AppHandle,
    application: State<'_, crate::session::ApplicationState>,
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
    let result = crate::ipc::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn rename_graph_resource(
    app: AppHandle,
    application: State<'_, crate::session::ApplicationState>,
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
    let result = crate::ipc::schema::application_event::resource_mutation_to_transport(&result);
    emit_application_resource_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn update_function_signature(
    app: AppHandle,
    application: State<'_, crate::session::ApplicationState>,
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
    let result = crate::ipc::schema::application_event::resource_mutation_to_transport(&result);
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
    error: crate::graph::resources::ResourceMutationApplicationError,
) -> CommandError {
    super::common::resource_mutation_to_command_error(error, "resource_revision_conflict")
}

fn map_graph_draft_save_error(
    error: crate::graph::resources::ResourceMutationApplicationError,
) -> CommandError {
    super::common::resource_mutation_to_command_error(error, "graph_save_rejected")
}
