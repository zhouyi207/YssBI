use std::collections::{BTreeMap, BTreeSet};
use yss_filesystem::{NormalizedRoot, RecoveryMarker, TransactionContext};
use yss_project_history::ResourceKey;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSession {
    pub instance_id: ProjectInstanceId,
    pub root: NormalizedRoot,
}

#[derive(Clone, Debug)]
pub struct ProjectTransactionContext {
    pub session: ProjectSession,
    pub operation_id: OperationId,
    pub affected_resources: Vec<ResourceKey>,
    pub expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
    pub expected_absent_resources: BTreeSet<ResourceKey>,
    pub recovery_marker: Option<RecoveryMarker>,
}

impl ProjectTransactionContext {
    pub(crate) fn filesystem_context(&self) -> TransactionContext {
        TransactionContext {
            root: self.session.root.clone(),
            transaction_id: yss_filesystem::TransactionId::from_uuid((self.operation_id).as_uuid()),
            recovery_marker: self.recovery_marker.clone(),
        }
    }
}
