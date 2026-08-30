#[cfg(test)]
use crate::schema::application_event::{
    GraphMutationResultDto, GraphProjectionReplacementDto, LifecycleInvalidationDto,
    LifecycleMutationKindDto, LifecycleMutationOutcomeDto, LifecycleMutationPhaseDto,
    LifecycleRecoveryDto, ProjectionStatusDto, ResourceMoveDto, ResourceMutationCommandResultDto,
};
use crate::schema::application_event::{
    LifecycleMutationResultDto, ProjectActivationResultDto, ResourceMutationResultDto,
};
use serde::{Deserialize, Serialize};
use yss_computation_settings::ComputationSettingsMutationReceipt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventProject {
    #[serde(rename_all = "camelCase")]
    ProjectLoaded {
        result: ProjectActivationResultDto,
    },
    ProjectCleared,
    #[serde(rename_all = "camelCase")]
    ProjectLifecycleCommitted {
        result: LifecycleMutationResultDto,
    },
    #[serde(rename_all = "camelCase")]
    GraphDelta {
        project_instance_id: String,
        delta: crate::schema::application_event::GraphDeltaEventDto<
            yss_graph_document_edit::GraphDocumentPatch,
        >,
    },
    #[serde(rename_all = "camelCase")]
    ResourceMutationCommitted {
        result: ResourceMutationResultDto,
    },
    #[serde(rename_all = "camelCase")]
    ProjectSaved {
        result: crate::schema::ProjectSaveResultDto,
    },
    #[serde(rename_all = "camelCase")]
    ComputationSettingsChanged {
        result: ComputationSettingsMutationReceipt,
    },
}

#[cfg(all(test, any()))]
mod tests {
    use super::*;

    #[test]
    fn project_loaded_event_carries_identity_and_activation_revision() {
        let result = ProjectActivationResultDto {
            path: "D:/projects/demo".into(),
            project_instance_id: "project-2".into(),
            activation_revision: 7,
        };
        let event = crate::event::Event::Project(EventProject::ProjectLoaded {
            result: result.clone(),
        });

        let value = serde_json::to_value(event).unwrap();
        assert_eq!(
            value["payload"]["payload"]["result"],
            serde_json::to_value(result).unwrap()
        );
    }

    #[test]
    fn lifecycle_committed_event_serializes_the_exact_direct_receipt() {
        let operation_id =
            yss_project_identity::OperationId::from_uuid(uuid::Uuid::from_u128(0x790));
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
    fn resource_lifecycle_delta_serializes_explicit_optional_states() {
        let operation_id =
            yss_project_identity::OperationId::from_uuid(uuid::Uuid::from_u128(0x780));
        let delta = crate::project::ResourceDeltaEvent {
            resource: crate::project::ResourceKey::Graph(
                yss_graph_document::GraphResourcePath::new("events/Created.yssbi-event").unwrap(),
            ),
            from_revision: yss_project_identity::ResourceRevision::INITIAL,
            to_revision: yss_project_identity::ResourceRevision::new(1),
            caused_by: Some(operation_id),
            payload: crate::project::history::ResourceDocumentPatch::ResourceLifecycle(
                crate::project::ResourceLifecyclePatch {
                    before: None,
                    after: Some(crate::project::ResourceLifecycleState {
                        revision: yss_project_identity::ResourceRevision::INITIAL,
                        path: "events/Created.yssbi-event".into(),
                        kind: crate::project::ResourceLifecycleKind::Event,
                        name: "Created".into(),
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
                    "kind": "resource_lifecycle",
                    "patch": {
                        "before": null,
                        "after": {
                            "revision": 0,
                            "path": "events/Created.yssbi-event",
                            "kind": "event",
                            "name": "Created"
                        }
                    }
                }
            })
        );
    }

    #[test]
    fn worksheet_document_delta_uses_common_resource_delta_wire() {
        let operation_id =
            yss_project_identity::OperationId::from_uuid(uuid::Uuid::from_u128(0x781));
        let delta = crate::project::ResourceDeltaEvent {
            resource: crate::project::ResourceKey::Worksheet(crate::project::WorksheetResourceKey(
                "worksheets/Sales Report.yssbi-worksheet".into(),
            )),
            from_revision: yss_project_identity::ResourceRevision::new(4),
            to_revision: yss_project_identity::ResourceRevision::new(5),
            caused_by: Some(operation_id),
            payload: crate::project::history::ResourceDocumentPatch::Worksheet(
                crate::project::WorksheetDocumentPatch {
                    before: crate::project::WorksheetDocumentState {
                        database_id: "database-before".into(),
                        chart_type: "histogram".into(),
                        encodings: yss_worksheet_document::WorksheetEncodings {
                            x: Some("region".into()),
                            y: None,
                        },
                    },
                    after: crate::project::WorksheetDocumentState {
                        database_id: "database-after".into(),
                        chart_type: "scatter".into(),
                        encodings: yss_worksheet_document::WorksheetEncodings {
                            x: Some("region".into()),
                            y: Some("revenue".into()),
                        },
                    },
                },
            ),
        };

        assert_eq!(
            serde_json::to_value(delta).unwrap(),
            serde_json::json!({
                "resource": {
                    "kind": "worksheet",
                    "key": "worksheets/Sales Report.yssbi-worksheet"
                },
                "fromRevision": 4,
                "toRevision": 5,
                "causedBy": operation_id,
                "payload": {
                    "kind": "worksheet",
                    "patch": {
                        "before": {
                            "databaseId": "database-before",
                            "chartType": "histogram",
                            "encodings": { "x": "region" }
                        },
                        "after": {
                            "databaseId": "database-after",
                            "chartType": "scatter",
                            "encodings": { "x": "region", "y": "revenue" }
                        }
                    }
                }
            })
        );
    }

    #[test]
    fn worksheet_lifecycle_delta_carries_rust_derived_name() {
        let operation_id =
            yss_project_identity::OperationId::from_uuid(uuid::Uuid::from_u128(0x782));
        let delta = crate::project::ResourceDeltaEvent {
            resource: crate::project::ResourceKey::Worksheet(crate::project::WorksheetResourceKey(
                "worksheets/Sales Report.yssbi-worksheet".into(),
            )),
            from_revision: yss_project_identity::ResourceRevision::INITIAL,
            to_revision: yss_project_identity::ResourceRevision::new(1),
            caused_by: Some(operation_id),
            payload: crate::project::history::ResourceDocumentPatch::ResourceLifecycle(
                crate::project::ResourceLifecyclePatch {
                    before: None,
                    after: Some(crate::project::ResourceLifecycleState {
                        revision: yss_project_identity::ResourceRevision::INITIAL,
                        path: "worksheets/Sales Report.yssbi-worksheet".into(),
                        kind: crate::project::ResourceLifecycleKind::Worksheet,
                        name: "Sales Report".into(),
                    }),
                },
            ),
        };

        assert_eq!(
            serde_json::to_value(delta).unwrap()["payload"],
            serde_json::json!({
                "kind": "resource_lifecycle",
                "patch": {
                    "before": null,
                    "after": {
                        "revision": 0,
                        "path": "worksheets/Sales Report.yssbi-worksheet",
                        "kind": "worksheet",
                        "name": "Sales Report"
                    }
                }
            })
        );
    }

    #[test]
    fn worksheet_move_uses_common_resource_move_wire() {
        let result = ResourceMutationResultDto {
            operation_id: yss_project_identity::OperationId::new(),
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 9,
            moves: vec![ResourceMoveDto {
                from: "worksheets/Old.yssbi-worksheet".into(),
                to: "worksheets/New.yssbi-worksheet".into(),
                kind: crate::project::ResourceLifecycleKind::Worksheet,
                name: "New".into(),
            }],
            deltas: vec![crate::project::ResourceDeltaEvent {
                resource: crate::project::ResourceKey::Worksheet(
                    crate::project::WorksheetResourceKey("worksheets/New.yssbi-worksheet".into()),
                ),
                from_revision: yss_project_identity::ResourceRevision::new(2),
                to_revision: yss_project_identity::ResourceRevision::new(3),
                caused_by: None,
                payload: crate::project::history::ResourceDocumentPatch::ResourceMove(
                    crate::project::ResourcePathMovePatch {
                        from: "worksheets/Old.yssbi-worksheet".into(),
                        to: "worksheets/New.yssbi-worksheet".into(),
                    },
                ),
            }],
            projection_replacements: Vec::new(),
            projection_status: ProjectionStatusDto::Complete {
                expected_graph_paths: Vec::new(),
            },
            history: Default::default(),
        };

        let wire = serde_json::to_value(result).unwrap();
        assert_eq!(wire["moves"][0]["kind"], "worksheet");
        assert_eq!(wire["deltas"][0]["payload"]["kind"], "resource_move");
    }

    #[test]
    fn resource_mutation_result_has_no_worksheet_side_channel() {
        let result = ResourceMutationResultDto {
            operation_id: yss_project_identity::OperationId::new(),
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 1,
            moves: Vec::new(),
            deltas: Vec::new(),
            projection_replacements: Vec::new(),
            projection_status: ProjectionStatusDto::Complete {
                expected_graph_paths: Vec::new(),
            },
            history: Default::default(),
        };
        let mut wire = serde_json::to_value(result).unwrap();
        wire.as_object_mut()
            .unwrap()
            .insert("worksheetDeltas".into(), serde_json::json!([]));

        assert!(serde_json::from_value::<ResourceMutationResultDto>(wire).is_err());
    }

    #[test]
    fn resource_mutation_result_serializes_explicit_graph_move_identity() {
        let result = ResourceMutationResultDto {
            operation_id: yss_project_identity::OperationId::new(),
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 9,
            moves: vec![ResourceMoveDto {
                from: "events/Old.yssbi-event".into(),
                to: "events/New.yssbi-event".into(),
                kind: crate::project::ResourceLifecycleKind::Event,
                name: "New".into(),
            }],
            deltas: Vec::new(),
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
            yss_project_identity::OperationId::from_uuid(uuid::Uuid::from_u128(0x778));
        let result = ResourceMutationResultDto {
            operation_id,
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 41,
            moves: Vec::new(),
            deltas: Vec::new(),
            projection_replacements: Vec::new(),
            projection_status: ProjectionStatusDto::Complete {
                expected_graph_paths: vec![
                    "events/Caller.yssbi-event".into(),
                    "functions/Observable.yssbi-function".into(),
                ],
            },
            history: crate::project::HistoryStatusDto {
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
    fn graph_delta_event_carries_project_identity() {
        let delta = crate::application::events::GraphDeltaEvent {
            graph_path: yss_graph_document::GraphResourcePath::new("events/Main.yssbi-event")
                .unwrap(),
            from_revision: yss_project_identity::ResourceRevision::INITIAL,
            to_revision: yss_project_identity::ResourceRevision::new(1),
            caused_by: Some(yss_project_identity::OperationId::from_uuid(
                uuid::Uuid::from_u128(0x401),
            )),
            payload: yss_graph_document_edit::GraphDocumentPatch {
                operations: Vec::new(),
            },
        };
        let projection = serde_json::from_value(serde_json::json!({
            "basis": {
                "graphPath": "events/Main.yssbi-event",
                "graphRevision": 1,
                "registryFingerprint": "0000000000000000000000000000000000000000000000000000000000000000",
                "resourceVersions": {},
            },
            "graphPath": "events/Main.yssbi-event",
            "sourceRevision": 1,
            "nodes": [],
            "connections": [],
            "diagnostics": [],
            "hasBlockingDiagnostics": false,
            "outcome": { "type": "success" },
        }))
        .unwrap();
        let result = GraphMutationResultDto {
            project_instance_id: "project-a".into(),
            delta: delta.clone(),
            projection_replacement: GraphProjectionReplacementDto {
                graph_path: "events/Main.yssbi-event".into(),
                projection,
                function_editor_projection: None,
            },
            history: Default::default(),
        };

        assert_eq!(
            serde_json::to_value(EventProject::GraphDelta {
                project_instance_id: "project-a".into(),
                delta: delta.clone(),
            })
            .unwrap(),
            serde_json::json!({
                "type": "GraphDelta",
                "payload": {
                    "projectInstanceId": "project-a",
                    "delta": serde_json::to_value(delta).unwrap(),
                }
            }),
        );

        let result_wire = serde_json::to_value(result).unwrap();
        assert_eq!(result_wire["projectInstanceId"], "project-a");
        let mut missing_identity = result_wire;
        missing_identity
            .as_object_mut()
            .unwrap()
            .remove("projectInstanceId");
        assert!(serde_json::from_value::<GraphMutationResultDto>(missing_identity).is_err());
    }

    #[test]
    fn resource_mutation_result_marks_incomplete_projection_paths_on_wire() {
        let operation_id =
            yss_project_identity::OperationId::from_uuid(uuid::Uuid::from_u128(0x779));
        let result = ResourceMutationResultDto {
            operation_id,
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 42,
            moves: Vec::new(),
            deltas: Vec::new(),
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
