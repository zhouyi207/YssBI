use std::future::Future;
use std::pin::Pin;
use thiserror::Error;

use super::{ProjectRootIdentity, ProjectRootIdentityState};

use yss_project_identity::ProjectInstanceId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRegistryRecord {
    id: Box<str>,
    name: Box<str>,
    path: Box<str>,
    created_at: Box<str>,
    last_opened_at: Option<Box<str>>,
    favorite: bool,
    root_identity: ProjectRootIdentity,
    root_identity_state: ProjectRootIdentityState,
}

impl ProjectRegistryRecord {
    pub fn new(
        id: Box<str>,
        name: Box<str>,
        path: Box<str>,
        created_at: Box<str>,
        last_opened_at: Option<Box<str>>,
        favorite: bool,
        root_identity: ProjectRootIdentity,
        root_identity_state: ProjectRootIdentityState,
    ) -> Self {
        Self {
            id,
            name,
            path,
            created_at,
            last_opened_at,
            favorite,
            root_identity,
            root_identity_state,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    pub fn last_opened_at(&self) -> Option<&str> {
        self.last_opened_at.as_deref()
    }

    pub const fn is_favorite(&self) -> bool {
        self.favorite
    }

    pub fn root_identity(&self) -> &ProjectRootIdentity {
        &self.root_identity
    }

    pub const fn root_identity_state(&self) -> ProjectRootIdentityState {
        self.root_identity_state
    }
}

pub type ProjectRegistryStoreFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum ProjectRegistryStoreError {
    #[error("project registry store is unavailable")]
    Unavailable,
    #[error("project registry store conflict")]
    Conflict,
    #[error("project registry storage failed")]
    StorageFailed,
}

pub trait ProjectRegistryStore: Send + Sync {
    fn load(
        &self,
    ) -> ProjectRegistryStoreFuture<
        '_,
        Result<Box<[ProjectRegistryRecord]>, ProjectRegistryStoreError>,
    >;

    fn upsert(
        &self,
        record: &ProjectRegistryRecord,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>>;

    fn remove(
        &self,
        project: &ProjectInstanceId,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>>;
}
