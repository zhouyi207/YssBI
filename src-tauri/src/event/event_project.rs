use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphProjectionReplacementDto {
    pub graph_path: String,
    pub projection: crate::node_system::analysis::EditorGraphProjectionDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMutationResultDto {
    pub delta: crate::node_system::document::GraphDeltaEvent<
        crate::node_system::document::GraphDocumentPatch,
    >,
    pub projection_replacement: GraphProjectionReplacementDto,
    pub history: crate::node_system::document::HistoryStatusDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectionStatusDto {
    Complete {
        expected_graph_paths: Vec<String>,
    },
    Incomplete {
        invalidated_graph_paths: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMoveDto {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksheetDeltaDto {
    pub id: String,
    pub before: Option<crate::project::WorksheetDocument>,
    pub after: Option<crate::project::WorksheetDocument>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMutationResultDto {
    pub project_instance_id: String,
    pub publication_revision: u64,
    pub moves: Vec<ResourceMoveDto>,
    pub deltas: Vec<crate::node_system::document::ResourceDeltaEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub worksheet_deltas: Vec<WorksheetDeltaDto>,
    pub projection_replacements: Vec<GraphProjectionReplacementDto>,
    pub projection_status: ProjectionStatusDto,
    pub history: crate::node_system::document::HistoryStatusDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventProject {
    #[serde(rename_all = "camelCase")]
    ProjectLoaded {
        path: Option<String>,
    },
    ProjectCleared,
    #[serde(rename_all = "camelCase")]
    GraphDelta {
        delta: crate::node_system::document::GraphDeltaEvent<
            crate::node_system::document::GraphDocumentPatch,
        >,
    },
    #[serde(rename_all = "camelCase")]
    ResourceMutationCommitted {
        result: ResourceMutationResultDto,
    },
    #[serde(rename_all = "camelCase")]
    ProjectSaved {
        result: crate::project::project_writers::ProjectSaveResultDto,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_mutation_result_serializes_explicit_graph_move_identity() {
        let result = ResourceMutationResultDto {
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 9,
            moves: vec![ResourceMoveDto {
                from: "events/Old.yssbi-event".into(),
                to: "events/New.yssbi-event".into(),
                kind: "event".into(),
                name: "New".into(),
            }],
            deltas: Vec::new(),
            worksheet_deltas: Vec::new(),
            projection_replacements: Vec::new(),
            projection_status: ProjectionStatusDto::Incomplete {
                invalidated_graph_paths: vec!["events/New.yssbi-event".into()],
            },
            history: Default::default(),
        };

        assert_eq!(
            serde_json::to_value(result).unwrap()["moves"],
            serde_json::json!([{
                "from": "events/Old.yssbi-event",
                "to": "events/New.yssbi-event",
                "kind": "event",
                "name": "New"
            }])
        );
    }

    #[test]
    fn resource_mutation_result_uses_explicit_atomic_wire_fields() {
        let result = ResourceMutationResultDto {
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 41,
            moves: Vec::new(),
            deltas: Vec::new(),
            worksheet_deltas: Vec::new(),
            projection_replacements: Vec::new(),
            projection_status: ProjectionStatusDto::Complete {
                expected_graph_paths: vec![
                    "events/Caller.yssbi-event".into(),
                    "functions/Observable.yssbi-function".into(),
                ],
            },
            history: crate::node_system::document::HistoryStatusDto {
                can_undo: true,
                can_redo: false,
            },
        };

        let wire = serde_json::to_value(result).unwrap();
        assert_eq!(wire["publicationRevision"], serde_json::json!(41));
        assert_eq!(
            wire,
            serde_json::json!({
                "projectInstanceId": "00000000-0000-0000-0000-000000000601",
                "publicationRevision": 41,
                "moves": [],
                "deltas": [],
                "projectionReplacements": [],
                "projectionStatus": {
                    "status": "complete",
                    "expectedGraphPaths": [
                        "events/Caller.yssbi-event",
                        "functions/Observable.yssbi-function"
                    ]
                },
                "history": {
                    "canUndo": true,
                    "canRedo": false,
                },
            })
        );
    }

    #[test]
    fn resource_mutation_result_marks_incomplete_projection_paths_on_wire() {
        let result = ResourceMutationResultDto {
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 42,
            moves: Vec::new(),
            deltas: Vec::new(),
            worksheet_deltas: Vec::new(),
            projection_replacements: Vec::new(),
            projection_status: ProjectionStatusDto::Incomplete {
                invalidated_graph_paths: vec!["functions/Observable.yssbi-function".into()],
            },
            history: Default::default(),
        };

        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::json!({
                "projectInstanceId": "00000000-0000-0000-0000-000000000601",
                "publicationRevision": 42,
                "moves": [],
                "deltas": [],
                "projectionReplacements": [],
                "projectionStatus": {
                    "status": "incomplete",
                    "invalidatedGraphPaths": ["functions/Observable.yssbi-function"]
                },
                "history": {
                    "canUndo": false,
                    "canRedo": false
                }
            })
        );
    }
}
