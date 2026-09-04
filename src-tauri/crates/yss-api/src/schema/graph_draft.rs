use serde::{Deserialize, Serialize};

use super::application_event::{
    GraphProjectionReplacementDto, graph_projection_replacement_to_transport,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditorSessionDto {
    pub document: yss_graph_document::GraphDocument,
    pub projection: crate::schema::editor_projection_types::EditorGraphProjectionDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDraftAcceptedDto {
    pub project_instance_id: String,
    pub graph_session_id: String,
    pub graph_path: String,
    pub accepted_revision: u64,
    pub request_generation: u64,
    pub operation_id: yss_project_identity::OperationId,
    pub document: yss_graph_document::GraphDocument,
    pub patch: yss_graph_document_edit::GraphDocumentPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphProjectionRequestDto {
    pub graph_session_id: String,
    pub graph_path: String,
    pub accepted_revision: u64,
    pub request_generation: u64,
    pub operation_id: yss_project_identity::OperationId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDraftSaveDto {
    pub project_instance_id: String,
    pub operation_id: yss_project_identity::OperationId,
    pub document: yss_graph_document::GraphDocument,
    pub projection_replacement: GraphProjectionReplacementDto,
    pub history: yss_project_history::HistoryStatusDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompileGraphDraftDto {
    pub source_hash: Box<str>,
    pub cache_hit: bool,
    pub document: yss_graph_document::GraphDocument,
    pub projection: crate::schema::editor_projection_types::EditorGraphProjectionDto,
}

pub(crate) fn compile_graph_draft_to_transport(
    receipt: &yss_application::graph_compile::CompileGraphDraftReceipt,
) -> CompileGraphDraftDto {
    CompileGraphDraftDto {
        source_hash: receipt
            .source_hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            .into(),
        cache_hit: receipt.cache_hit,
        document: receipt.document.clone(),
        projection: crate::schema::editor_projection::map_editor_projection(&receipt.projection),
    }
}

pub(crate) fn graph_editor_session_to_transport(
    document: &yss_graph_document::GraphDocument,
    projection: &yss_application::editor_projection::EditorProjectionModel,
) -> GraphEditorSessionDto {
    GraphEditorSessionDto {
        document: document.clone(),
        projection: crate::schema::editor_projection::map_editor_projection(projection),
    }
}

pub(crate) fn graph_draft_accepted_to_transport(
    update: &yss_application::resource_mutation::GraphDraftAccepted,
    project_instance_id: &yss_project_identity::ProjectInstanceId,
    graph_session_id: String,
    graph_path: &yss_graph_document::GraphResourcePath,
    accepted_revision: u64,
    request_generation: u64,
    operation_id: yss_project_identity::OperationId,
) -> GraphDraftAcceptedDto {
    GraphDraftAcceptedDto {
        project_instance_id: project_instance_id.to_string(),
        graph_session_id,
        graph_path: graph_path.as_str().to_owned(),
        accepted_revision,
        request_generation,
        operation_id,
        document: update.document.clone(),
        patch: update.patch.clone(),
    }
}

pub(crate) fn graph_draft_save_to_transport(
    saved: &yss_application::resource_mutation::GraphDraftSave,
) -> GraphDraftSaveDto {
    GraphDraftSaveDto {
        project_instance_id: saved.project_instance_id.to_string(),
        operation_id: saved.operation_id,
        document: saved.document.clone(),
        projection_replacement: graph_projection_replacement_to_transport(
            &saved.projection_replacement,
        ),
        history: yss_project_history::HistoryStatusDto {
            can_undo: saved.history.can_undo,
            can_redo: saved.history.can_redo,
        },
    }
}
