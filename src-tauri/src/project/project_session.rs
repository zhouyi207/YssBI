use crate::node_system::document::ResourceKey;
use crate::project::NormalizedProjectRoot;
use crate::project::{OperationId, ResourceRevision};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectInstanceId(String);

impl ProjectInstanceId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub(crate) fn from_existing(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ProjectInstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProjectInstanceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSession {
    pub instance_id: ProjectInstanceId,
    pub root: NormalizedProjectRoot,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectRecoveryMarker {
    state: Arc<Mutex<Option<String>>>,
}

impl ProjectRecoveryMarker {
    pub fn mark(&self, message: impl Into<String>) {
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(message.into());
    }

    pub(crate) fn boundary_recovering(&self) -> (std::sync::MutexGuard<'_, Option<String>>, bool) {
        match self.state.lock() {
            Ok(state) => (state, false),
            Err(error) => (error.into_inner(), true),
        }
    }

    pub(crate) fn clear_poison(&self) {
        self.state.clear_poison();
    }

    #[cfg(test)]
    pub(crate) fn boundary_is_available(&self) -> bool {
        self.state.try_lock().is_ok()
    }

    pub fn clear(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }

    pub fn error(&self) -> Option<crate::project::ProjectFilesystemError> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .map(
                |message| crate::project::ProjectFilesystemError::ProjectRecoveryRequired {
                    message,
                },
            )
    }
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
