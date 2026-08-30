use std::collections::{BTreeMap, BTreeSet};
use yss_project_filesystem::{
    NormalizedProjectRoot, ProjectFilesystemTransactionContext, ProjectRecoveryMarker,
};
use yss_project_history::ResourceKey;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSession {
    pub instance_id: ProjectInstanceId,
    pub root: NormalizedProjectRoot,
}

#[derive(Clone, Debug)]
pub struct ProjectTransactionContext {
    pub session: ProjectSession,
    pub operation_id: OperationId,
    pub affected_resources: Vec<ResourceKey>,
    pub expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
    pub expected_absent_resources: BTreeSet<ResourceKey>,
    pub recovery_marker: Option<ProjectRecoveryMarker>,
}

impl ProjectTransactionContext {
    pub(crate) fn filesystem_context(&self) -> ProjectFilesystemTransactionContext {
        ProjectFilesystemTransactionContext {
            root: self.session.root.clone(),
            operation_id: self.operation_id,
            recovery_marker: self.recovery_marker.clone(),
        }
    }
}
