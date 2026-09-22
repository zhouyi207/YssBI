use serde::{Deserialize, Serialize};
use yss_project_registry_contract::ProjectRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActivationResultDto {
    pub path: String,
    pub project_instance_id: String,
    pub activation_revision: u64,
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
    pub operation_id: yss_project_identity::OperationId,
    pub kind: LifecycleMutationKindDto,
    pub old_project_instance_id: Option<String>,
    pub new_project_instance_id: Option<String>,
    pub phase: LifecycleMutationPhaseDto,
    pub outcome: LifecycleMutationOutcomeDto,
    pub record: Option<ProjectRecord>,
    pub path: Option<String>,
    pub recovery: Option<LifecycleRecoveryDto>,
    pub invalidation: LifecycleInvalidationDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphProjectionReplacementDto {
    pub graph_path: String,
    pub projection: crate::editor_projection::EditorGraphProjectionDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_editor_projection:
        Option<yss_function_editor_projection::FunctionEditorProjection>,
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
    pub kind: yss_project_history::ResourceLifecycleKind,
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
    pub operation_id: yss_project_identity::OperationId,
    pub project_instance_id: String,
    pub publication_revision: u64,
    pub moves: Vec<ResourceMoveDto>,
    pub deltas: Vec<yss_project_history::ResourceDeltaEvent>,
    pub projection_replacements: Vec<GraphProjectionReplacementDto>,
    pub projection_status: ProjectionStatusDto,
}
