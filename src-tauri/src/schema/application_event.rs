use crate::application::events::{
    ApplicationEvent, CommittedResourceMutation, LifecycleRecoveryAction, ProjectLifecycleKind,
    ProjectLifecycleOutcome, ProjectLifecyclePhase, ResourceProjectionStatus,
};
use crate::event::EventProject;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActivationResultDto {
    pub path: String,
    pub project_instance_id: String,
    pub activation_revision: u64,
}

pub(crate) fn project_activation_to_transport(
    activation: &crate::application::project_query::ProjectActivation,
) -> ProjectActivationResultDto {
    ProjectActivationResultDto {
        path: activation.path.clone(),
        project_instance_id: activation.project_instance_id.to_string(),
        activation_revision: activation.activation_revision,
    }
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
    pub operation_id: crate::project::OperationId,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphProjectionReplacementDto {
    pub graph_path: String,
    pub projection: crate::schema::editor_projection_types::EditorGraphProjectionDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_editor_projection:
        Option<crate::schema::editor_projection_types::FunctionEditorProjectionDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDeltaEventDto<T> {
    pub graph_path: String,
    pub from_revision: crate::project::ResourceRevision,
    pub to_revision: crate::project::ResourceRevision,
    pub caused_by: Option<crate::project::OperationId>,
    pub payload: T,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMutationResultDto {
    pub project_instance_id: String,
    pub delta: GraphDeltaEventDto<crate::graph::document::GraphDocumentPatch>,
    pub projection_replacement: GraphProjectionReplacementDto,
    pub history: crate::project::HistoryStatusDto,
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
    pub kind: crate::project::ResourceLifecycleKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceMutationCommandResultDto<T> {
    pub data: T,
    pub mutation: ResourceMutationResultDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceMutationResultDto {
    pub operation_id: crate::project::OperationId,
    pub project_instance_id: String,
    pub publication_revision: u64,
    pub moves: Vec<ResourceMoveDto>,
    pub deltas: Vec<crate::project::ResourceDeltaEvent>,
    pub projection_replacements: Vec<GraphProjectionReplacementDto>,
    pub projection_status: ProjectionStatusDto,
    pub history: crate::project::HistoryStatusDto,
}

pub type ApplicationEventDto = EventProject;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("application event transport mapping failed")]
pub struct TransportMappingError;

#[derive(Debug, thiserror::Error)]
pub enum GraphMutationTransportError {
    #[error("editor projection transport mapping failed")]
    Projection(#[source] crate::schema::editor_projection::TransportMappingError),
}

pub fn graph_mutation_to_transport(
    result: &crate::application::events::GraphMutationResult,
) -> Result<GraphMutationResultDto, GraphMutationTransportError> {
    Ok(GraphMutationResultDto {
        project_instance_id: result.project_instance_id.to_string(),
        delta: graph_delta_to_transport(&result.delta),
        projection_replacement: GraphProjectionReplacementDto {
            graph_path: result.projection_replacement.graph_path.to_string(),
            projection: crate::schema::editor_projection::map_editor_projection(
                &result.projection_replacement.projection,
            )
            .map_err(GraphMutationTransportError::Projection)?,
            function_editor_projection: result
                .projection_replacement
                .function_editor_projection
                .as_ref()
                .map(crate::schema::editor_projection_types::FunctionEditorProjectionDto::from),
        },
        history: crate::project::HistoryStatusDto {
            can_undo: result.history.can_undo,
            can_redo: result.history.can_redo,
        },
    })
}

pub(crate) fn graph_delta_to_transport(
    delta: &crate::application::events::GraphDeltaEvent<crate::graph::document::GraphDocumentPatch>,
) -> GraphDeltaEventDto<crate::graph::document::GraphDocumentPatch> {
    GraphDeltaEventDto {
        graph_path: delta.graph_path.as_str().to_owned(),
        from_revision: delta.from_revision,
        to_revision: delta.to_revision,
        caused_by: delta.caused_by,
        payload: delta.payload.clone(),
    }
}

pub fn application_event_to_transport(
    event: &ApplicationEvent,
) -> Result<ApplicationEventDto, TransportMappingError> {
    match event {
        ApplicationEvent::ProjectLifecycle(event) => Ok(EventProject::ProjectLifecycleCommitted {
            result: project_lifecycle_to_transport(event),
        }),
        ApplicationEvent::ResourceCommitted(mutation) => {
            Ok(EventProject::ResourceMutationCommitted {
                result: resource_mutation_to_transport(mutation),
            })
        }
    }
}

pub(crate) fn project_lifecycle_to_transport(
    event: &crate::application::events::ProjectLifecycleApplicationEvent,
) -> LifecycleMutationResultDto {
    LifecycleMutationResultDto {
        operation_id: event.operation_id,
        kind: lifecycle_kind_to_transport(event.kind),
        old_project_instance_id: event
            .old_project_instance_id
            .as_ref()
            .map(ToString::to_string),
        new_project_instance_id: event
            .new_project_instance_id
            .as_ref()
            .map(ToString::to_string),
        phase: lifecycle_phase_to_transport(event.phase),
        outcome: lifecycle_outcome_to_transport(event.outcome),
        record: event.record.clone(),
        path: event.path.as_deref().map(str::to_owned),
        recovery: event.recovery.as_ref().map(recovery_to_transport),
        invalidation: LifecycleInvalidationDto {
            project: event.invalidation.project,
            registry: event.invalidation.registry,
        },
    }
}

fn lifecycle_kind_to_transport(kind: ProjectLifecycleKind) -> LifecycleMutationKindDto {
    match kind {
        ProjectLifecycleKind::SaveAs => LifecycleMutationKindDto::SaveAs,
        ProjectLifecycleKind::Create => LifecycleMutationKindDto::Create,
        ProjectLifecycleKind::Delete => LifecycleMutationKindDto::Delete,
        ProjectLifecycleKind::RegistryCleanup => LifecycleMutationKindDto::RegistryCleanup,
        ProjectLifecycleKind::Load => LifecycleMutationKindDto::Load,
        ProjectLifecycleKind::Clear => LifecycleMutationKindDto::Clear,
    }
}

fn lifecycle_phase_to_transport(phase: ProjectLifecyclePhase) -> LifecycleMutationPhaseDto {
    match phase {
        ProjectLifecyclePhase::DestinationCommitted => {
            LifecycleMutationPhaseDto::DestinationCommitted
        }
        ProjectLifecyclePhase::RegistryCommitted => LifecycleMutationPhaseDto::RegistryCommitted,
        ProjectLifecyclePhase::AuthorityCommitted => LifecycleMutationPhaseDto::AuthorityCommitted,
    }
}

fn lifecycle_outcome_to_transport(outcome: ProjectLifecycleOutcome) -> LifecycleMutationOutcomeDto {
    match outcome {
        ProjectLifecycleOutcome::Committed => LifecycleMutationOutcomeDto::Committed,
        ProjectLifecycleOutcome::RegistryFailed => LifecycleMutationOutcomeDto::RegistryFailed,
        ProjectLifecycleOutcome::ActivationFailed => LifecycleMutationOutcomeDto::ActivationFailed,
        ProjectLifecycleOutcome::RegistryPending => LifecycleMutationOutcomeDto::RegistryPending,
    }
}

fn recovery_to_transport(
    recovery: &crate::application::events::LifecycleRecovery,
) -> LifecycleRecoveryDto {
    LifecycleRecoveryDto {
        required: recovery.required,
        action: match recovery.action {
            LifecycleRecoveryAction::RemoveRegistryRecord => "removeRegistryRecord",
            LifecycleRecoveryAction::CleanupRegistry => "cleanupRegistry",
            LifecycleRecoveryAction::ActivateDestination => "activateDestination",
        }
        .to_owned(),
        path: recovery.path.as_deref().map(str::to_owned),
        identity: recovery.identity.as_deref().map(str::to_owned),
    }
}

pub(crate) fn resource_mutation_to_transport(
    mutation: &CommittedResourceMutation,
) -> ResourceMutationResultDto {
    ResourceMutationResultDto {
        operation_id: mutation.operation_id,
        project_instance_id: mutation.project_instance_id.to_string(),
        publication_revision: mutation.publication_revision,
        moves: mutation
            .moves
            .iter()
            .map(|resource_move| ResourceMoveDto {
                from: resource_move.from.to_string(),
                to: resource_move.to.to_string(),
                kind: resource_move.kind,
                name: resource_move.name.to_string(),
            })
            .collect(),
        deltas: mutation.deltas.clone(),
        projection_replacements: Vec::new(),
        projection_status: projection_status_to_transport(&mutation.projection_status),
        history: crate::project::HistoryStatusDto {
            can_undo: mutation.history.can_undo,
            can_redo: mutation.history.can_redo,
        },
    }
}

fn projection_status_to_transport(status: &ResourceProjectionStatus) -> ProjectionStatusDto {
    match status {
        ResourceProjectionStatus::Complete {
            expected_graph_paths,
        } => ProjectionStatusDto::Complete {
            expected_graph_paths: expected_graph_paths
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect::<Vec<_>>(),
        },
        ResourceProjectionStatus::Incomplete {
            invalidated_graph_paths,
        } => ProjectionStatusDto::Incomplete {
            invalidated_graph_paths: invalidated_graph_paths
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect::<Vec<_>>(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::application_event_to_transport;
    use crate::application::events::{
        ApplicationEvent, CommittedResourceMutation, HistoryStatus, LifecycleInvalidation,
        LifecycleRecovery, LifecycleRecoveryAction, ProjectLifecycleApplicationEvent,
        ProjectLifecycleKind, ProjectLifecycleOutcome, ProjectLifecyclePhase, ResourceMove,
        ResourceProjectionStatus,
    };
    use crate::project::ResourceLifecycleKind;
    use crate::project::{OperationId, ProjectInstanceId};
    use crate::schema::application_event::ResourceMutationResultDto;
    use serde_json::json;

    #[test]
    fn project_lifecycle_fact_maps_to_the_existing_event_wire_shape() {
        let operation_id = OperationId::from_uuid(uuid::Uuid::from_u128(0x790));
        let old_project_instance_id = ProjectInstanceId::new();
        let new_project_instance_id = ProjectInstanceId::new();
        let old_project_instance_id_wire = old_project_instance_id.to_string();
        let new_project_instance_id_wire = new_project_instance_id.to_string();
        let event = ApplicationEvent::ProjectLifecycle(ProjectLifecycleApplicationEvent {
            operation_id,
            kind: ProjectLifecycleKind::SaveAs,
            old_project_instance_id: Some(old_project_instance_id),
            new_project_instance_id: Some(new_project_instance_id),
            phase: ProjectLifecyclePhase::AuthorityCommitted,
            outcome: ProjectLifecycleOutcome::Committed,
            record: None,
            path: Some("C:/projects/copy/metadata.yssbi".into()),
            recovery: Some(LifecycleRecovery {
                required: true,
                action: LifecycleRecoveryAction::RemoveRegistryRecord,
                path: Some("C:/projects/copy/metadata.yssbi".into()),
                identity: None,
            }),
            invalidation: LifecycleInvalidation {
                project: true,
                registry: true,
            },
        });

        let wire = serde_json::to_value(
            application_event_to_transport(&event).expect("staged event mapping is infallible"),
        )
        .expect("existing event wire should serialize");

        assert_eq!(
            wire,
            json!({
                "type": "ProjectLifecycleCommitted",
                "payload": {
                    "result": {
                        "operationId": operation_id,
                        "kind": "saveAs",
                        "oldProjectInstanceId": old_project_instance_id_wire,
                        "newProjectInstanceId": new_project_instance_id_wire,
                        "phase": "authorityCommitted",
                        "outcome": "committed",
                        "record": null,
                        "path": "C:/projects/copy/metadata.yssbi",
                        "recovery": {
                            "required": true,
                            "action": "removeRegistryRecord",
                            "path": "C:/projects/copy/metadata.yssbi",
                            "identity": null,
                        },
                        "invalidation": {
                            "project": true,
                            "registry": true,
                        },
                    }
                }
            })
        );
    }

    #[test]
    fn committed_resource_fact_preserves_wire_fields_and_required_identity() {
        let operation_id = OperationId::from_uuid(uuid::Uuid::from_u128(0x780));
        let event = ApplicationEvent::ResourceCommitted(CommittedResourceMutation {
            operation_id,
            project_instance_id: ProjectInstanceId::new(),
            publication_revision: 41,
            moves: vec![ResourceMove {
                from: "events/Before.yssbi-event".into(),
                to: "events/After.yssbi-event".into(),
                kind: ResourceLifecycleKind::Event,
                name: "After".into(),
            }],
            deltas: Vec::new(),
            projection_status: ResourceProjectionStatus::Complete {
                expected_graph_paths: vec![
                    yss_graph_document::GraphResourcePath::new("events/After.yssbi-event")
                        .expect("fixture graph path is valid"),
                ],
            },
            history: HistoryStatus {
                can_undo: true,
                can_redo: false,
            },
        });

        let wire = serde_json::to_value(
            application_event_to_transport(&event).expect("staged event mapping is infallible"),
        )
        .expect("existing event wire should serialize");
        let project_instance_id = wire["payload"]["result"]["projectInstanceId"]
            .as_str()
            .expect("project identity should remain a string")
            .to_owned();

        assert_eq!(
            wire["payload"]["result"],
            json!({
                "operationId": operation_id,
                "projectInstanceId": project_instance_id,
                "publicationRevision": 41,
                "moves": [{
                    "from": "events/Before.yssbi-event",
                    "to": "events/After.yssbi-event",
                    "kind": "event",
                    "name": "After",
                }],
                "deltas": [],
                "projectionReplacements": [],
                "projectionStatus": {
                    "status": "complete",
                    "expectedGraphPaths": ["events/After.yssbi-event"],
                },
                "history": {
                    "canUndo": true,
                    "canRedo": false,
                },
            })
        );

        let mut missing_identity = wire["payload"]["result"].clone();
        missing_identity
            .as_object_mut()
            .expect("resource result is an object")
            .remove("projectInstanceId");
        assert!(serde_json::from_value::<ResourceMutationResultDto>(missing_identity).is_err());
    }
}
