use super::common::{mutation_conflict_to_command_error, parse_graph_path};
use crate::error::CommandError;
use crate::event::{Event, EventProject, GraphMutationResultDto, emit_project_event};
use crate::graph_document::NodeId;
use crate::node_system::analysis::EditorGraphProjectionDto;
use crate::node_system::document::{ClipboardSubgraphDto, EditorGraphMutationDto, MutationRequest};
use crate::project::{ProjectInstanceId, ProjectState};
use tauri::{AppHandle, State};

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
) -> Result<EditorGraphProjectionDto, CommandError> {
    hydrate_editor_graph_from_state(state.inner(), project_instance_id, graph_path, &locale)
}

pub(crate) fn export_graph_subgraph_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, CommandError> {
    state
        .export_editor_subgraph(
            &project_instance_id,
            &parse_graph_path(graph_path)?,
            node_ids,
        )
        .map_err(|error| mutation_conflict_to_command_error(error, "graph_revision_conflict"))
}

#[tauri::command]
pub fn export_graph_subgraph(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, CommandError> {
    export_graph_subgraph_from_state(state.inner(), project_instance_id, graph_path, node_ids)
}

pub(super) fn parse_editor_mutation_request(
    request: serde_json::Value,
) -> Result<MutationRequest<EditorGraphMutationDto>, CommandError> {
    serde_json::from_value(request.clone()).map_err(|_| {
        let code = if is_create_node_descriptor_shape_error(&request) {
            "catalog_descriptor_invalid"
        } else {
            "invalid_editor_mutation"
        };
        CommandError::expected(code)
    })
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
        serde_json::from_value::<crate::node_system::catalog::NodeCreationDescriptor>(
            descriptor.clone(),
        )
        .is_err()
    })
}

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
        .map_err(|error| mutation_conflict_to_command_error(error, "graph_revision_conflict"))?;
    if !result.delta.payload.operations.is_empty() {
        emit(Event::Project(EventProject::GraphDelta {
            project_instance_id: result.project_instance_id.clone(),
            delta: result.delta.clone(),
        }));
    }
    Ok(result)
}

#[tauri::command]
pub fn mutate_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
    request: serde_json::Value,
) -> Result<GraphMutationResultDto, CommandError> {
    mutate_graph_document_with_emitter(
        state.inner(),
        project_instance_id,
        graph_path,
        &locale,
        request,
        |event| emit_project_event(&app, event),
    )
}
