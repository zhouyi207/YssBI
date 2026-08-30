use super::ProjectState;
use crate::project::{ProjectFilesystemError, ProjectSession};
use yss_database_contract::DatabaseDecl;
use yss_project_identity::ProjectInstanceId;

#[derive(Debug, thiserror::Error)]
pub enum ProjectDatabaseError {
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error("stale database revision")]
    StaleDatabaseRevision,
    #[error("database already exists")]
    DatabaseAlreadyExists,
    #[error("database not found")]
    DatabaseNotFound,
    #[error("database name is invalid")]
    InvalidName,
    #[error("database name already exists")]
    NameConflict,
    #[error("database operation failed: {0}")]
    Operation(String),
}

impl ProjectDatabaseError {
    pub fn operation(error: impl std::fmt::Display) -> Self {
        Self::Operation(error.to_string())
    }

    pub const fn command_code(&self) -> Option<&'static str> {
        match self {
            Self::Project(error) => Some(error.code()),
            Self::StaleDatabaseRevision => Some("stale_database_revision"),
            Self::DatabaseAlreadyExists => Some("database_already_exists"),
            Self::DatabaseNotFound => Some("database_not_found"),
            Self::InvalidName => Some("invalid_database_name"),
            Self::NameConflict => Some("database_name_conflict"),
            Self::Operation(_) => None,
        }
    }

    pub const fn recovery_required(&self) -> bool {
        match self {
            Self::Project(error) => error.recovery_required(),
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DatabaseAuthorityToken {
    project_instance_id: String,
    project_session_id: yss_project_identity::ProjectSessionId,
    database_id: String,
    database_revision: u64,
}

impl ProjectState {
    pub(crate) fn reserve_database_operation(
        &self,
        project_instance_id: &yss_project_identity::ProjectInstanceId,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<yss_project_operation::ProjectOperationReservation, ProjectDatabaseError> {
        self.ensure_project_operational()?;
        self.reserve_resource_operation(project_instance_id, operation_id)
            .map_err(Into::into)
    }

    /// Runs a read/query operation against a detached database snapshot.
    pub(crate) fn validate_database_project_identity(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<ProjectSession, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project instance changed".into(),
            });
        }
        self.validate_project_session(&session)?;
        Ok(session)
    }

    pub(crate) fn acquire_database_publication_authority(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<crate::project::ProjectActivationToken, ProjectFilesystemError> {
        let authority = self.project_activation.acquire();
        self.validate_database_project_identity(project_instance_id)?;
        Ok(authority)
    }

    /// Captures only the durable declaration authority needed by the
    /// Application/Database two-owner mutation handoff. The physical
    /// `DatabaseInstance` never crosses this seam.
    pub(crate) fn prepare_database_mutation_authority(
        &self,
        project_instance_id: &ProjectInstanceId,
        id: &str,
        expected_revision: yss_project_identity::ResourceRevision,
    ) -> Result<DatabaseAuthorityToken, ProjectDatabaseError> {
        let session = self.validate_database_project_identity(project_instance_id)?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project instance changed".into(),
            }
            .into());
        }
        let data = self.project_data.read().unwrap();
        let store = self.project_store.read().unwrap();
        let revisions = self.database_authority_revisions.read().unwrap();
        if !data.databases.contains_key(id) {
            return Err(ProjectDatabaseError::DatabaseNotFound);
        }
        let database_revision = revisions.get(id).copied().ok_or_else(|| {
            ProjectDatabaseError::from(ProjectFilesystemError::StaleProjectLifecycle {
                message: "database authority is missing".into(),
            })
        })?;
        if database_revision != expected_revision.get() {
            return Err(ProjectDatabaseError::StaleDatabaseRevision);
        }
        Ok(DatabaseAuthorityToken {
            project_instance_id: publication.project_instance_id.clone(),
            project_session_id: store.project_session_id.clone(),
            database_id: id.to_owned(),
            database_revision,
        })
    }

    pub(crate) fn commit_database_declaration_for_application(
        &self,
        project_instance_id: &ProjectInstanceId,
        token: DatabaseAuthorityToken,
        after: DatabaseDecl,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<crate::project::project_writers::ProjectResourceMutationFacts, ProjectDatabaseError>
    {
        let session = self.validate_database_project_identity(project_instance_id)?;
        self.commit_database_declaration(&session, token, after, operation_id)
    }

    fn validate_database_authority(
        publication: &super::project_state::MutationPublication,
        session: &ProjectSession,
        current_session_id: &yss_project_identity::ProjectSessionId,
        revisions: &std::collections::HashMap<String, u64>,
        token: &DatabaseAuthorityToken,
        id: &str,
    ) -> Result<(), ProjectDatabaseError> {
        if publication.project_instance_id != session.instance_id.as_str()
            || publication.project_instance_id != token.project_instance_id
            || current_session_id != &token.project_session_id
            || token.database_id != id
            || revisions.get(id).copied() != Some(token.database_revision)
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "database authority conflict".into(),
            }
            .into());
        }
        Ok(())
    }

    fn next_database_revision(
        id: &str,
        retained: yss_project_identity::ResourceRevision,
    ) -> Result<yss_project_identity::ResourceRevision, ProjectDatabaseError> {
        retained.checked_next().map_err(|error| {
            ProjectFilesystemError::ResourceRevisionOverflow {
                resource: format!("databases/{id}"),
                retained: error.retained,
            }
            .into()
        })
    }

    fn publish_database_delta(
        &self,
        publication: &mut super::project_state::MutationPublication,
        revisions: &mut std::collections::HashMap<String, u64>,
        id: &str,
        from_revision: yss_project_identity::ResourceRevision,
        to_revision: yss_project_identity::ResourceRevision,
        operation_id: yss_project_identity::OperationId,
        publication_advance: super::project_state::PreparedPublicationAdvance,
        before: Option<DatabaseDecl>,
        after: Option<DatabaseDecl>,
    ) -> crate::project::project_writers::ProjectResourceMutationFacts {
        use yss_project_history::{
            DatabaseDocumentPatch, DatabaseResourceKey, ResourceDeltaEvent, ResourceDocumentPatch,
            ResourceKey,
        };

        let publication_revision = publication.commit_prepared(publication_advance);
        revisions.insert(id.to_string(), to_revision.get());
        crate::project::project_writers::ProjectResourceMutationFacts::new(
            operation_id,
            ProjectInstanceId::from_existing(publication.project_instance_id.clone()),
            publication_revision,
            Vec::new(),
            vec![ResourceDeltaEvent {
                resource: ResourceKey::Database(DatabaseResourceKey(
                    format!("databases/{id}").into(),
                )),
                from_revision,
                to_revision,
                caused_by: Some(operation_id),
                payload: ResourceDocumentPatch::Database(DatabaseDocumentPatch { before, after }),
            }],
            crate::project::project_writers::ProjectProjectionStatus::Complete {
                expected_graph_paths: Vec::new().into(),
            },
            {
                let history = self.history.read().unwrap().status();
                crate::project::project_writers::ProjectHistoryStatus {
                    can_undo: history.can_undo,
                    can_redo: history.can_redo,
                }
            },
        )
    }

    pub(crate) fn commit_database_declaration(
        &self,
        session: &ProjectSession,
        token: DatabaseAuthorityToken,
        after: DatabaseDecl,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<crate::project::project_writers::ProjectResourceMutationFacts, ProjectDatabaseError>
    {
        let id = token.database_id.as_str();
        if after.id.as_str() != id {
            return Err(ProjectDatabaseError::DatabaseNotFound);
        }
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        let store = self.project_store.read().unwrap();
        let mut revisions = self.database_authority_revisions.write().unwrap();
        Self::validate_database_authority(
            &publication,
            session,
            &store.project_session_id,
            &revisions,
            &token,
            id,
        )?;
        let before = data
            .databases
            .get(id)
            .cloned()
            .ok_or(ProjectDatabaseError::DatabaseNotFound)?;
        let publication_advance = publication.prepare_resource_revision()?;
        let from_revision = yss_project_identity::ResourceRevision::new(token.database_revision);
        let to_revision = Self::next_database_revision(id, from_revision)?;
        data.databases.insert(id.to_owned(), after.clone());
        Ok(self.publish_database_delta(
            &mut publication,
            &mut revisions,
            id,
            from_revision,
            to_revision,
            operation_id,
            publication_advance,
            Some(before),
            Some(after),
        ))
    }

    /// Commit a newly imported declaration without installing a physical
    /// database instance in Project. Physical state is owned by the captured
    /// Database session; this method only linearizes durable Project facts.
    pub(crate) fn commit_database_declaration_add_for_application(
        &self,
        project_instance_id: &ProjectInstanceId,
        after: DatabaseDecl,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<crate::project::project_writers::ProjectResourceMutationFacts, ProjectDatabaseError>
    {
        let session = self.validate_database_project_identity(project_instance_id)?;
        let id = after.id.as_str().to_owned();
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        let mut revisions = self.database_authority_revisions.write().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project instance changed".into(),
            }
            .into());
        }
        if data.databases.contains_key(&id) || revisions.contains_key(&id) {
            return Err(ProjectDatabaseError::DatabaseAlreadyExists);
        }
        let publication_advance = publication.prepare_resource_revision()?;
        let from_revision = yss_project_identity::ResourceRevision::INITIAL;
        let to_revision = Self::next_database_revision(&id, from_revision)?;
        data.databases.insert(id.clone(), after.clone());
        Ok(self.publish_database_delta(
            &mut publication,
            &mut revisions,
            &id,
            from_revision,
            to_revision,
            operation_id,
            publication_advance,
            None,
            Some(after),
        ))
    }

    pub(crate) fn commit_database_declaration_delete_for_application(
        &self,
        project_instance_id: &ProjectInstanceId,
        id: &str,
        expected_revision: yss_project_identity::ResourceRevision,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<crate::project::project_writers::ProjectResourceMutationFacts, ProjectDatabaseError>
    {
        let session = self.validate_database_project_identity(project_instance_id)?;
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        let mut revisions = self.database_authority_revisions.write().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project instance changed".into(),
            }
            .into());
        }
        let current_revision = revisions
            .get(id)
            .copied()
            .ok_or(ProjectDatabaseError::DatabaseNotFound)?;
        if current_revision != expected_revision.get() {
            return Err(ProjectDatabaseError::StaleDatabaseRevision);
        }
        let before = data
            .databases
            .remove(id)
            .ok_or(ProjectDatabaseError::DatabaseNotFound)?;
        let publication_advance = publication.prepare_resource_revision()?;
        let from_revision = yss_project_identity::ResourceRevision::new(current_revision);
        let to_revision = Self::next_database_revision(id, from_revision)?;
        Ok(self.publish_database_delta(
            &mut publication,
            &mut revisions,
            id,
            from_revision,
            to_revision,
            operation_id,
            publication_advance,
            Some(before),
            None,
        ))
    }
}
