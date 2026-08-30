#[cfg(all(test, any()))]
use super::common::mutation_conflict_to_command_error;
use super::common::parse_graph_path;
use crate::application::execution::{ApplicationState, SessionCaptureError};
use crate::application::graph_open::{OpenGraphApplicationError, OpenGraphRequest};
use crate::error::CommandError;
#[cfg(all(test, any()))]
use crate::event::emit_project_event;
use crate::event::{Event, EventProject, emit_project_event_result};
#[cfg(all(test, any()))]
use crate::project::ProjectState;
use crate::project::{MutationRequest, ProjectInstanceId};
use crate::schema::application_event::GraphMutationResultDto;
use crate::schema::editor_projection_types::EditorGraphProjectionDto;
use crate::schema::graph_clipboard::ClipboardSubgraphDto;
use crate::schema::graph_mutation::EditorGraphMutationDto;
use tauri::{AppHandle, State};
use yss_graph_document::NodeId;
#[cfg(all(test, any()))]
use yss_graph_editor::ClipboardSubgraph;
use yss_graph_editor::EditorGraphMutation;

#[cfg(all(test, any()))]
pub(super) fn hydrate_editor_graph_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: &str,
) -> Result<EditorGraphProjectionDto, CommandError> {
    state
        .graph_projection_for_project(&project_instance_id, &parse_graph_path(graph_path)?, locale)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn hydrate_editor_graph(
    state: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
) -> Result<EditorGraphProjectionDto, CommandError> {
    let graph_path = parse_graph_path(graph_path)?;
    let receipt = state
        .open_graph(OpenGraphRequest::new(
            project_instance_id,
            graph_path,
            0,
            locale,
        ))
        .map_err(open_graph_command_error)?;
    crate::schema::editor_projection::map_editor_projection(receipt.projection())
        .map_err(|error| CommandError::diagnosed("editor_projection_mapping_failed", error))
}

fn open_graph_command_error(error: OpenGraphApplicationError) -> CommandError {
    match error {
        OpenGraphApplicationError::SessionCapture(error) => session_capture_command_error(error),
        OpenGraphApplicationError::SessionChanged => {
            CommandError::expected("stale_project_lifecycle")
        }
        OpenGraphApplicationError::Project(error) => {
            CommandError::diagnosed("graph_open_failed", error)
        }
        OpenGraphApplicationError::Database(error) => {
            CommandError::diagnosed("database_catalog_failed", error)
        }
        OpenGraphApplicationError::Contract(error) => {
            CommandError::diagnosed("graph_contract_failed", error)
        }
        OpenGraphApplicationError::Materialization(error) => {
            CommandError::diagnosed("graph_materialization_failed", error)
        }
        OpenGraphApplicationError::Projection(error) => {
            CommandError::diagnosed("editor_projection_failed", error)
        }
    }
}

fn session_capture_command_error(error: SessionCaptureError) -> CommandError {
    match error {
        SessionCaptureError::Inactive => CommandError::expected("stale_project_lifecycle"),
        SessionCaptureError::Replacing => {
            CommandError::expected("project_lifecycle_admission_closed")
        }
        SessionCaptureError::Recovering => CommandError::expected("project_recovery_required")
            .with_details(super::common::RecoveryRequiredDetails {
                recovery_required: true,
            }),
    }
}

#[cfg(all(test, any()))]
pub(crate) fn export_graph_subgraph_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraph, CommandError> {
    state
        .export_editor_subgraph(
            &project_instance_id,
            &parse_graph_path(graph_path)?,
            node_ids,
        )
        .map_err(mutation_conflict_to_command_error)
}

#[tauri::command]
pub fn export_graph_subgraph(
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, CommandError> {
    application
        .export_graph_subgraph(project_instance_id, parse_graph_path(graph_path)?, node_ids)
        .map(ClipboardSubgraphDto::from)
        .map_err(map_editor_resource_error)
}

pub(super) fn parse_editor_mutation_request(
    request: serde_json::Value,
) -> Result<MutationRequest<EditorGraphMutation>, CommandError> {
    let request =
        serde_json::from_value::<MutationRequest<EditorGraphMutationDto>>(request.clone())
            .map_err(|_| {
                let code = if is_create_node_descriptor_shape_error(&request) {
                    "catalog_descriptor_invalid"
                } else {
                    "invalid_editor_mutation"
                };
                CommandError::expected(code)
            })?;
    let payload = request
        .payload
        .try_into()
        .map_err(|_| CommandError::expected("invalid_editor_mutation"))?;
    Ok(MutationRequest::new(
        request.resource,
        request.base_revision,
        request.operation_id,
        payload,
    ))
}

fn is_create_node_descriptor_shape_error(request: &serde_json::Value) -> bool {
    let Some(mutation) = request
        .get("payload")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    if mutation.get("type").and_then(serde_json::Value::as_str) != Some("createNode") {
        return false;
    }
    let Some(create) = mutation
        .get("payload")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    if create.contains_key("parameters") {
        return true;
    }
    create.get("descriptor").is_none_or(|descriptor| {
        serde_json::from_value::<crate::schema::catalog::NodeCreationDescriptorDto>(
            descriptor.clone(),
        )
        .is_err()
    })
}

#[cfg(all(test, any()))]
pub(crate) fn mutate_graph_document_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: &str,
    request: serde_json::Value,
    mut emit: impl FnMut(Event),
) -> Result<GraphMutationResultDto, CommandError> {
    let request = parse_editor_mutation_request(request)?;
    let result = state
        .apply_editor_graph_mutation(
            &project_instance_id,
            &parse_graph_path(graph_path)?,
            locale,
            request,
        )
        .map_err(mutation_conflict_to_command_error)?;
    if !result.delta.payload.operations.is_empty() {
        emit(Event::Project(EventProject::GraphDelta {
            project_instance_id: result.project_instance_id.clone(),
            delta: crate::schema::application_event::graph_delta_to_transport(&result.delta),
        }));
    }
    Ok(result)
}

#[tauri::command]
pub fn mutate_graph_document(
    app: AppHandle,
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
    request: serde_json::Value,
) -> Result<GraphMutationResultDto, CommandError> {
    let parsed_request = parse_editor_mutation_request(request)?;
    let result = application
        .mutate_graph_document(
            project_instance_id,
            parse_graph_path(graph_path)?,
            locale,
            parsed_request,
        )
        .map_err(map_editor_resource_error)?;
    if !result.delta.payload.operations.is_empty() {
        emit_project_event_result(
            &app,
            &Event::Project(EventProject::GraphDelta {
                project_instance_id: result.project_instance_id.to_string(),
                delta: crate::schema::application_event::graph_delta_to_transport(&result.delta),
            }),
        )
        .map_err(|error| CommandError::diagnosed("graph_event_emit_failed", error))?;
    }
    crate::schema::application_event::graph_mutation_to_transport(&result)
        .map_err(|error| CommandError::diagnosed("editor_projection_mapping_failed", error))
}

fn map_editor_resource_error(
    error: crate::application::resource_mutation::ResourceMutationApplicationError,
) -> CommandError {
    super::common::resource_mutation_to_command_error(error, "graph_revision_conflict")
}
