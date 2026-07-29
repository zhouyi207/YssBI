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
    pub operation_id: crate::node_system::document::OperationId,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LifecycleMutationKindDto {
    SaveAs,
    Create,
    Delete,
    RegistryCleanup,
    Load,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LifecycleMutationPhaseDto {
    DestinationCommitted,
    TombstoneCommitted,
    RegistryCommitted,
    AuthorityCommitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LifecycleMutationOutcomeDto {
    Committed,
    RegistryFailed,
    ActivationFailed,
    RegistryPending,
    CleanupPending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleRecoveryDto {
    pub required: bool,
    pub action: String,
    pub path: Option<String>,
    pub identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleInvalidationDto {
    pub project: bool,
    pub registry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleMutationResultDto {
    pub operation_id: crate::node_system::document::OperationId,
    pub kind: LifecycleMutationKindDto,
    pub old_project_instance_id: Option<String>,
    pub new_project_instance_id: Option<String>,
    pub phase: LifecycleMutationPhaseDto,
    pub outcome: LifecycleMutationOutcomeDto,
    pub record: Option<crate::project::ProjectRecord>,
    pub path: Option<String>,
    pub recovery: Option<LifecycleRecoveryDto>,
    pub invalidation: LifecycleInvalidationDto,
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
    ProjectLifecycleCommitted {
        result: LifecycleMutationResultDto,
    },
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
    fn lifecycle_committed_event_serializes_the_exact_direct_receipt() {
        let operation_id =
            crate::node_system::document::OperationId::from_uuid(uuid::Uuid::from_u128(0x790));
        let result = LifecycleMutationResultDto {
            operation_id,
            kind: LifecycleMutationKindDto::SaveAs,
            old_project_instance_id: Some("old-project".into()),
            new_project_instance_id: Some("new-project".into()),
            phase: LifecycleMutationPhaseDto::AuthorityCommitted,
            outcome: LifecycleMutationOutcomeDto::Committed,
            record: None,
            path: Some("C:/projects/copy/metadata.yssbi".into()),
            recovery: None,
            invalidation: LifecycleInvalidationDto {
                project: true,
                registry: true,
            },
        };
        let event = EventProject::ProjectLifecycleCommitted {
            result: result.clone(),
        };

        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({
                "type": "ProjectLifecycleCommitted",
                "payload": {
                    "result": serde_json::to_value(result).unwrap()
                }
            })
        );
    }

    #[test]
    fn resource_mutation_result_serializes_required_top_level_operation_id() {
        let operation_id =
            crate::node_system::document::OperationId::from_uuid(uuid::Uuid::from_u128(0x777));
        let result = ResourceMutationResultDto {
            operation_id,
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 1,
            moves: Vec::new(),
            deltas: Vec::new(),
            worksheet_deltas: Vec::new(),
            projection_replacements: Vec::new(),
            projection_status: ProjectionStatusDto::Complete {
                expected_graph_paths: Vec::new(),
            },
            history: Default::default(),
        };

        let wire = serde_json::to_value(result).unwrap();
        assert_eq!(wire["operationId"], serde_json::json!(operation_id));
    }

    #[test]
    fn graph_resource_lifecycle_delta_serializes_explicit_optional_states() {
        let operation_id =
            crate::node_system::document::OperationId::from_uuid(uuid::Uuid::from_u128(0x780));
        let delta = crate::node_system::document::ResourceDeltaEvent {
            resource: crate::node_system::document::ResourceKey::Graph(
                crate::node_system::document::GraphResourcePath(
                    "events/Created.yssbi-event".into(),
                ),
            ),
            from_revision: crate::node_system::document::ResourceRevision::INITIAL,
            to_revision: crate::node_system::document::ResourceRevision::new(1),
            caused_by: Some(operation_id),
            payload: crate::node_system::document::ResourceDocumentPatch::GraphResourceLifecycle(
                crate::node_system::document::GraphResourceLifecyclePatch {
                    before: None,
                    after: Some(crate::node_system::document::GraphResourceLifecycleState {
                        revision: crate::node_system::document::ResourceRevision::INITIAL,
                        path: "events/Created.yssbi-event".into(),
                        kind: crate::node_system::document::GraphResourceLifecycleKind::Event,
                    }),
                },
            ),
        };

        assert_eq!(
            serde_json::to_value(delta).unwrap(),
            serde_json::json!({
                "resource": { "kind": "graph", "key": "events/Created.yssbi-event" },
                "fromRevision": 0,
                "toRevision": 1,
                "causedBy": operation_id,
                "payload": {
                    "kind": "graph_resource_lifecycle",
                    "patch": {
                        "before": null,
                        "after": {
                            "revision": 0,
                            "path": "events/Created.yssbi-event",
                            "kind": "event"
                        }
                    }
                }
            })
        );
    }

    #[test]
    fn resource_mutation_result_serializes_explicit_graph_move_identity() {
        let result = ResourceMutationResultDto {
            operation_id: crate::node_system::document::OperationId::new(),
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
        let operation_id =
            crate::node_system::document::OperationId::from_uuid(uuid::Uuid::from_u128(0x778));
        let result = ResourceMutationResultDto {
            operation_id,
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
                "operationId": operation_id,
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
        let operation_id =
            crate::node_system::document::OperationId::from_uuid(uuid::Uuid::from_u128(0x779));
        let result = ResourceMutationResultDto {
            operation_id,
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
                "operationId": operation_id,
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
