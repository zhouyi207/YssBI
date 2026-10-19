//! Canonical record and persistence port for the project registry.
//!
//! This crate owns the data shared by Project and persistence adapters.
//! Registry workflows and concrete storage implementations remain in their
//! respective layers.

use std::future::Future;
use std::pin::Pin;
use thiserror::Error;
use yss_project_identity::{ProjectRegistrationId, ProjectRootIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectRootIdentityState {
    Valid,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRecord {
    pub id: ProjectRegistrationId,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub last_opened_at: Option<String>,
    pub root_identity: ProjectRootIdentity,
    pub root_identity_state: ProjectRootIdentityState,
}

impl ProjectRecord {
    pub fn valid_root_identity(&self) -> Option<&ProjectRootIdentity> {
        (self.root_identity_state == ProjectRootIdentityState::Valid
            && !self.root_identity.as_str().is_empty())
        .then_some(&self.root_identity)
    }
}

pub type ProjectRegistryStoreFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum ProjectRegistryStoreError {
    #[error("project registry storage failed")]
    StorageFailed,
}

pub trait ProjectRegistryStore: Send + Sync {
    /// Paths use the canonical metadata path admitted by the Registry workflow.
    fn get_by_path(
        &self,
        path: &str,
    ) -> ProjectRegistryStoreFuture<'_, Result<Option<ProjectRecord>, ProjectRegistryStoreError>>;

    fn load(
        &self,
    ) -> ProjectRegistryStoreFuture<'_, Result<Box<[ProjectRecord]>, ProjectRegistryStoreError>>;

    fn upsert(
        &self,
        record: &ProjectRecord,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>>;
}

#[cfg(test)]
mod tests {
    use super::{ProjectRecord, ProjectRootIdentityState};
    use yss_project_identity::{ProjectRegistrationId, ProjectRootIdentity};

    #[test]
    fn invalid_or_empty_root_identity_never_matches_a_project_root() {
        let mut invalid = ProjectRecord {
            id: ProjectRegistrationId::from_existing("registration-1".into()),
            name: "Example".into(),
            path: "C:/projects/example/metadata.yssbi".into(),
            created_at: "17".into(),
            last_opened_at: Some("19".into()),
            root_identity: ProjectRootIdentity::from_canonical("native-root-3".into()),
            root_identity_state: ProjectRootIdentityState::Invalid,
        };
        assert_eq!(invalid.valid_root_identity(), None);

        invalid.root_identity_state = ProjectRootIdentityState::Valid;
        invalid.root_identity = ProjectRootIdentity::from_canonical(String::new());
        assert_eq!(invalid.valid_root_identity(), None);
    }
}
