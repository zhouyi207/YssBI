use crate::project::GraphProjectionReplacementDto;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditorSessionDto {
    pub document: yss_graph_document::GraphDocument,
    pub projection: crate::editor_projection::EditorGraphProjectionDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDraftTransformDto {
    pub changed: bool,
    pub document: yss_graph_document::GraphDocument,
    pub projection: crate::editor_projection::EditorGraphProjectionDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDraftSaveDto {
    pub project_instance_id: String,
    pub resource_revision: yss_project_identity::ResourceRevision,
    pub document: yss_graph_document::GraphDocument,
    pub projection_replacement: GraphProjectionReplacementDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CompileGraphDraftDto {
    Ready {
        artifact_id: Box<str>,
        cache_hit: bool,
        projection: crate::editor_projection::EditorGraphProjectionDto,
    },
    Blocked {
        projection: crate::editor_projection::EditorGraphProjectionDto,
    },
}
