//! Canonical record and persistence port for the project registry.
//!
//! This crate owns the data shared by Project, transport, and persistence
//! adapters. Registry workflows and concrete storage implementations remain in
//! their respective layers.

use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use thiserror::Error;
use yss_project_identity::{ProjectRegistrationId, ProjectRootIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectRootIdentityState {
    Valid,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRecord {
    pub id: ProjectRegistrationId,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub last_opened_at: Option<String>,
    pub is_favorite: bool,
    pub root_identity: ProjectRootIdentity,
    pub root_identity_state: ProjectRootIdentityState,
}

impl ProjectRecord {
    pub fn deletion_identity(&self) -> Option<&ProjectRootIdentity> {
        (self.root_identity_state == ProjectRootIdentityState::Valid
            && !self.root_identity.as_str().is_empty())
        .then_some(&self.root_identity)
    }
}

pub type ProjectRegistryStoreFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum ProjectRegistryStoreError {
    #[error("project registry store is unavailable")]
    Unavailable,
    #[error("project registry storage failed")]
    StorageFailed,
}

pub trait ProjectRegistryStore: Send + Sync {
    fn load(
        &self,
    ) -> ProjectRegistryStoreFuture<'_, Result<Box<[ProjectRecord]>, ProjectRegistryStoreError>>;

    fn upsert(
        &self,
        record: &ProjectRecord,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>>;

    fn remove(
        &self,
        registration: &ProjectRegistrationId,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>>;
}

#[cfg(test)]
mod tests {
    use super::{ProjectRecord, ProjectRootIdentityState};
    use serde_json::json;
    use yss_project_identity::{ProjectRegistrationId, ProjectRootIdentity};

    fn record() -> ProjectRecord {
        ProjectRecord {
            id: ProjectRegistrationId::from_existing("registration-1".into()),
            name: "Example".into(),
            path: "C:/projects/example/metadata.yssbi".into(),
            created_at: "17".into(),
            last_opened_at: Some("19".into()),
            is_favorite: true,
            root_identity: ProjectRootIdentity::from_canonical("native-root-3".into()),
            root_identity_state: ProjectRootIdentityState::Valid,
        }
    }

    #[test]
    fn record_preserves_the_existing_camel_case_wire_shape() {
        assert_eq!(
            serde_json::to_value(record()).unwrap(),
            json!({
                "id": "registration-1",
                "name": "Example",
                "path": "C:/projects/example/metadata.yssbi",
                "createdAt": "17",
                "lastOpenedAt": "19",
                "isFavorite": true,
                "rootIdentity": "native-root-3",
                "rootIdentityState": "valid",
            })
        );
    }

    #[test]
    fn invalid_or_empty_root_identity_is_never_authorized_for_deletion() {
        let mut invalid = record();
        invalid.root_identity_state = ProjectRootIdentityState::Invalid;
        assert_eq!(invalid.deletion_identity(), None);

        invalid.root_identity_state = ProjectRootIdentityState::Valid;
        invalid.root_identity = ProjectRootIdentity::from_canonical(String::new());
        assert_eq!(invalid.deletion_identity(), None);
    }

    #[test]
    fn unknown_record_fields_are_rejected() {
        let mut value = serde_json::to_value(record()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("legacyIdentity".into(), json!("duplicate"));

        assert!(serde_json::from_value::<ProjectRecord>(value).is_err());
    }
}
