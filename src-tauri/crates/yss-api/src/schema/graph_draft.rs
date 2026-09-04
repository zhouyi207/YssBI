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
pub struct GraphDraftTransformDto {
    pub changed: bool,
    pub document: yss_graph_document::GraphDocument,
    pub projection: crate::schema::editor_projection_types::EditorGraphProjectionDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDraftSaveDto {
    pub project_instance_id: String,
    pub operation_id: yss_project_identity::OperationId,
    pub resource_revision: yss_project_identity::ResourceRevision,
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

pub(crate) fn graph_draft_transform_to_transport(
    update: &yss_application::resource_mutation::GraphDraftTransform,
) -> GraphDraftTransformDto {
    GraphDraftTransformDto {
        changed: update.changed,
        document: update.document.clone(),
        projection: crate::schema::editor_projection::map_editor_projection(
            &update.projection_replacement.projection,
        ),
    }
}

pub(crate) fn graph_draft_save_to_transport(
    saved: &yss_application::resource_mutation::GraphDraftSave,
) -> GraphDraftSaveDto {
    GraphDraftSaveDto {
        project_instance_id: saved.project_instance_id.to_string(),
        operation_id: saved.operation_id,
        resource_revision: saved.resource_revision,
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
