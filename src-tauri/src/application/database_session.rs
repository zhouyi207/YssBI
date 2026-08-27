#![allow(
    dead_code,
    reason = "staged until Application session composition is installed"
)]

use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use super::ApplicationSession;
use crate::database::error::DatabaseError;
use crate::database::runtime::{DatabaseRuntimeRegistry, DatabaseRuntimeSession};
use crate::database_contract::DatabaseSessionOpenRequest;
use crate::database_contract::{
    DatabaseDecl, DatabaseDeclarationObservationSet, DatabaseSessionIdentity,
    DatabaseSessionOpenRequestError,
};
use crate::node_system::ProjectSessionId;
use crate::project::{NormalizedProjectRoot, ProjectInstanceId};

/// Owned Project facts used to open one Database runtime session. The Database
/// module receives only the converted contract request, never this type.
#[derive(Clone, Debug)]
pub(crate) struct ProjectDatabaseSessionFacts {
    project_instance_id: ProjectInstanceId,
    project_session_id: ProjectSessionId,
    generation: NonZeroU64,
    root: Option<NormalizedProjectRoot>,
    declarations: Arc<[DatabaseDecl]>,
    observations: DatabaseDeclarationObservationSet,
}

impl ProjectDatabaseSessionFacts {
    pub(crate) fn new(
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
        generation: NonZeroU64,
        root: Option<NormalizedProjectRoot>,
        declarations: Arc<[DatabaseDecl]>,
        observations: DatabaseDeclarationObservationSet,
    ) -> Self {
        Self {
            project_instance_id,
            project_session_id,
            generation,
            root,
            declarations,
            observations,
        }
    }

    pub(crate) fn from_captured_session(
        session: &ApplicationSession,
        generation: NonZeroU64,
        root: Option<NormalizedProjectRoot>,
        declarations: Arc<[DatabaseDecl]>,
        observations: DatabaseDeclarationObservationSet,
    ) -> Self {
        Self::new(
            session.project_instance_id().clone(),
            session.project_session_id().clone(),
            generation,
            root,
            declarations,
            observations,
        )
    }

    pub(crate) fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }
}

#[derive(Debug, Error)]
pub(crate) enum DatabaseSessionApplicationError {
    #[error("project database session identity is empty")]
    EmptyProjectSession,
    #[error("project database session facts are invalid")]
    InvalidFacts(#[source] DatabaseSessionOpenRequestError),
    #[error("database runtime session could not be opened")]
    Open(#[source] DatabaseError),
}

pub(crate) fn prepare_database_session(
    facts: &ProjectDatabaseSessionFacts,
) -> Result<Arc<DatabaseRuntimeSession>, DatabaseSessionApplicationError> {
    if facts.project_session_id.as_str().is_empty() {
        return Err(DatabaseSessionApplicationError::EmptyProjectSession);
    }
    let request = DatabaseSessionOpenRequest::new(
        DatabaseSessionIdentity::from_existing(facts.project_session_id.as_str().into()),
        facts.generation,
        facts
            .root
            .as_ref()
            .map(|root| PathBuf::from(root.as_path())),
        Arc::clone(&facts.declarations),
        facts.observations.clone(),
    );
    request
        .validate()
        .map_err(DatabaseSessionApplicationError::InvalidFacts)?;
    DatabaseRuntimeRegistry::new()
        .open_session(request)
        .map(Arc::new)
        .map_err(DatabaseSessionApplicationError::Open)
}
