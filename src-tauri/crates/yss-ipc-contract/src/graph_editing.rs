use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditVersionDto {
    pub session_id: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditingStateDto {
    pub version: GraphEditVersionDto,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GraphActivityDto {
    Changed {
        project_instance_id: String,
        graph_path: String,
        editing: GraphEditingStateDto,
    },
    Execution {
        project_instance_id: String,
        event: crate::execution::RunEventDto,
    },
    Resync {
        project_instance_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditorSessionDto {
    pub editing: GraphEditingStateDto,
    pub document: yss_graph_document::GraphDocument,
    pub projection: crate::editor_projection::EditorGraphProjectionDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEditorSyncResponseDto {
    pub project_instance_id: String,
    pub graph_path: String,
    pub locale: String,
    pub changed: bool,
    pub resource_revision: Option<yss_project_identity::ResourceRevision>,
    pub function_editor_projection:
        Option<yss_function_editor_projection::FunctionEditorProjection>,
    pub update: GraphEditorDeliveryDto,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GraphEditorDeliveryDto {
    Snapshot {
        cursor: String,
        data: serde_json::Value,
    },
    Delta {
        base_cursor: String,
        cursor: String,
        changes: Vec<GraphProjectionChangeDto>,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GraphProjectionChangeDto {
    Set {
        path: Vec<String>,
        value: serde_json::Value,
    },
    Remove {
        path: Vec<String>,
    },
}
