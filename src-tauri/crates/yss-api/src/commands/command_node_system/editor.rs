use super::common::parse_graph_path;
use crate::error::CommandError;
use crate::schema::graph_clipboard::ClipboardSubgraphDto;
use crate::schema::graph_draft::{
    CompileGraphDraftDto, GraphDraftUpdateDto, GraphEditorSessionDto,
};
use crate::schema::graph_mutation::EditorGraphMutationDto;
use tauri::State;
use yss_application::execution::{ApplicationState, SessionCaptureError};
use yss_application::graph_open::{OpenGraphApplicationError, OpenGraphRequest};
use yss_graph_document::NodeId;
use yss_graph_editor::EditorGraphMutation;
use yss_project_identity::ProjectInstanceId;

#[tauri::command]
pub fn hydrate_editor_graph(
    state: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
) -> Result<GraphEditorSessionDto, CommandError> {
    let graph_path = parse_graph_path(graph_path)?;
    let receipt = state
        .open_graph(OpenGraphRequest::new(
            project_instance_id,
            graph_path,
            0,
            locale,
        ))
        .map_err(open_graph_command_error)?;
    Ok(
        crate::schema::graph_draft::graph_editor_session_to_transport(
            receipt.document(),
            receipt.projection(),
        ),
    )
}

#[tauri::command]
pub fn compile_graph_draft(
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
    document: yss_graph_document::GraphDocument,
) -> Result<CompileGraphDraftDto, CommandError> {
    let receipt = yss_application::graph_compile::compile_graph_draft(
        &application,
        project_instance_id,
        parse_graph_path(graph_path)?,
        document,
        &locale,
    )
    .map_err(compile_graph_draft_error)?;
    Ok(crate::schema::graph_draft::compile_graph_draft_to_transport(&receipt))
}

fn compile_graph_draft_error(
    error: yss_application::graph_compile::CompileGraphDraftError,
) -> CommandError {
    use yss_application::graph_compile::CompileGraphDraftError;
    match error {
        CompileGraphDraftError::SessionCapture(error) => session_capture_command_error(error),
        CompileGraphDraftError::ProjectIdentityMismatch => {
            CommandError::expected("stale_project_lifecycle")
        }
        CompileGraphDraftError::GraphUnavailable => CommandError::expected("graph_not_loaded"),
        CompileGraphDraftError::InvalidDocument(_) => CommandError::expected("graph_draft_invalid"),
        CompileGraphDraftError::Project(error) => {
            crate::commands::project_failure::application_project_command_error(error)
        }
        CompileGraphDraftError::ProjectFacts(error) => {
            CommandError::diagnosed("catalog_project_read_failed", error)
        }
        CompileGraphDraftError::Database(error) => {
            CommandError::diagnosed("database_catalog_failed", error)
        }
        CompileGraphDraftError::Contract(error) => {
            CommandError::diagnosed("graph_contract_failed", error)
        }
        CompileGraphDraftError::Compilation(error) => {
            CommandError::diagnosed("graph_draft_compile_failed", error)
        }
        CompileGraphDraftError::Projection(error) => {
            CommandError::diagnosed("editor_projection_failed", error)
        }
        CompileGraphDraftError::SessionChanged(error) => {
            CommandError::diagnosed("resource_session_changed", error)
        }
    }
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

#[tauri::command]
pub fn export_graph_subgraph(
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    document: yss_graph_document::GraphDocument,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, CommandError> {
    application
        .export_graph_draft_subgraph(
            project_instance_id,
            parse_graph_path(graph_path)?,
            document,
            node_ids,
        )
        .map(ClipboardSubgraphDto::from)
        .map_err(map_editor_resource_error)
}

pub(super) fn parse_editor_mutation(
    mutation: serde_json::Value,
) -> Result<EditorGraphMutation, CommandError> {
    let mutation =
        serde_json::from_value::<EditorGraphMutationDto>(mutation.clone()).map_err(|_| {
            let code = if is_create_node_descriptor_shape_error(&mutation) {
                "catalog_descriptor_invalid"
            } else {
                "invalid_editor_mutation"
            };
            CommandError::expected(code)
        })?;
    mutation
        .try_into()
        .map_err(|_| CommandError::expected("invalid_editor_mutation"))
}

fn is_create_node_descriptor_shape_error(request: &serde_json::Value) -> bool {
    let Some(mutation) = request.as_object() else {
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

#[tauri::command]
pub fn transform_graph_draft(
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
    document: yss_graph_document::GraphDocument,
    mutation: serde_json::Value,
) -> Result<GraphDraftUpdateDto, CommandError> {
    let mutation = parse_editor_mutation(mutation)?;
    let result = application
        .transform_graph_draft(
            project_instance_id,
            parse_graph_path(graph_path)?,
            locale,
            document,
            mutation,
        )
        .map_err(map_editor_resource_error)?;
    Ok(crate::schema::graph_draft::graph_draft_update_to_transport(
        &result,
    ))
}

fn map_editor_resource_error(
    error: yss_application::resource_mutation::ResourceMutationApplicationError,
) -> CommandError {
    super::common::resource_mutation_to_command_error(error, "graph_draft_rejected")
}
