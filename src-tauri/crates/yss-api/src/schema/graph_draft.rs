use serde::{Deserialize, Serialize};

use super::application_event::{
    GraphProjectionReplacementDto, GraphProjectionTransportError,
    graph_projection_replacement_to_transport,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditorSessionDto {
    pub document: yss_graph_document::GraphDocument,
    pub projection: crate::schema::editor_projection_types::EditorGraphProjectionDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDraftUpdateDto {
    pub document: yss_graph_document::GraphDocument,
    pub patch: yss_graph_document_edit::GraphDocumentPatch,
    pub projection_replacement: GraphProjectionReplacementDto,
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

pub(crate) fn graph_editor_session_to_transport(
    graph_path: &yss_graph_document::GraphResourcePath,
    document: &yss_graph_document::GraphDocument,
    projection: &yss_application::editor_projection::EditorProjectionModel,
) -> Result<GraphEditorSessionDto, GraphProjectionTransportError> {
    let _ = graph_path;
    crate::schema::editor_projection::map_editor_projection(projection)
        .map_err(GraphProjectionTransportError::Projection)
        .map(|projection| GraphEditorSessionDto {
            document: document.clone(),
            projection,
        })
}

pub(crate) fn graph_draft_update_to_transport(
    update: &yss_application::resource_mutation::GraphDraftUpdate,
) -> Result<GraphDraftUpdateDto, GraphProjectionTransportError> {
    Ok(GraphDraftUpdateDto {
        document: update.document.clone(),
        patch: update.patch.clone(),
        projection_replacement: graph_projection_replacement_to_transport(
            &update.projection_replacement,
        )?,
    })
}

pub(crate) fn graph_draft_save_to_transport(
    saved: &yss_application::resource_mutation::GraphDraftSave,
) -> Result<GraphDraftSaveDto, GraphProjectionTransportError> {
    Ok(GraphDraftSaveDto {
        project_instance_id: saved.project_instance_id.to_string(),
        operation_id: saved.operation_id,
        document: saved.document.clone(),
        projection_replacement: graph_projection_replacement_to_transport(
            &saved.projection_replacement,
        )?,
        history: yss_project_history::HistoryStatusDto {
            can_undo: saved.history.can_undo,
            can_redo: saved.history.can_redo,
        },
    })
}
