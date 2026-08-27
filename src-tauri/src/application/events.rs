use crate::graph_document::GraphResourcePath;
use crate::node_system::document::{ResourceDeltaEvent, ResourceLifecycleKind};
use crate::project::{OperationId, ProjectInstanceId, ProjectRecord};

/// Low-rate cross-owner facts owned by Application.
///
/// This type is deliberately not serializable and carries no Tauri delivery
/// handle. The production event route remains the existing event adapter until
/// the named promotion task activates this staged owner.
#[derive(Debug, Clone, PartialEq)]
pub enum ApplicationEvent {
    ProjectLifecycle(ProjectLifecycleApplicationEvent),
    ResourceCommitted(CommittedResourceMutation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLifecycleApplicationEvent {
    pub operation_id: OperationId,
    pub kind: ProjectLifecycleKind,
    pub old_project_instance_id: Option<ProjectInstanceId>,
    pub new_project_instance_id: Option<ProjectInstanceId>,
    pub phase: ProjectLifecyclePhase,
    pub outcome: ProjectLifecycleOutcome,
    pub record: Option<ProjectRecord>,
    pub path: Option<Box<str>>,
    pub recovery: Option<LifecycleRecovery>,
    pub invalidation: LifecycleInvalidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectLifecycleKind {
    SaveAs,
    Create,
    Delete,
    RegistryCleanup,
    Load,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectLifecyclePhase {
    DestinationCommitted,
    RegistryCommitted,
    AuthorityCommitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectLifecycleOutcome {
    Committed,
    RegistryFailed,
    ActivationFailed,
    RegistryPending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleRecovery {
    pub required: bool,
    pub action: LifecycleRecoveryAction,
    pub path: Option<Box<str>>,
    pub identity: Option<Box<str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleRecoveryAction {
    RemoveRegistryRecord,
    CleanupRegistry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleInvalidation {
    pub project: bool,
    pub registry: bool,
}

/// The committed resource facts needed by the low-rate mutation event.
///
/// Editor projection replacements remain owned by the current production
/// event route in this staging slice. They are intentionally not represented
/// by a schema or UI type here; the later promotion task supplies the final
/// Application-owned projection facts in the same atomic cutover.
#[derive(Debug, Clone, PartialEq)]
pub struct CommittedResourceMutation {
    pub operation_id: OperationId,
    pub project_instance_id: ProjectInstanceId,
    pub publication_revision: u64,
    pub moves: Vec<ResourceMove>,
    pub deltas: Vec<ResourceDeltaEvent>,
    pub projection_status: ResourceProjectionStatus,
    pub history: HistoryStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceMove {
    pub from: Box<str>,
    pub to: Box<str>,
    pub kind: ResourceLifecycleKind,
    pub name: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceProjectionStatus {
    Complete {
        expected_graph_paths: Vec<GraphResourcePath>,
    },
    Incomplete {
        invalidated_graph_paths: Vec<GraphResourcePath>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryStatus {
    pub can_undo: bool,
    pub can_redo: bool,
}
