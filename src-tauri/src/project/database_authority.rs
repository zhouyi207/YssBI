use super::ProjectState;
#[cfg(test)]
use crate::database::*;
use crate::database_contract::DatabaseDecl;
#[cfg(test)]
use crate::database_contract::DatabaseId;
#[cfg(test)]
use crate::project::ProjectFilesystemLeaseSet;
use crate::project::{ProjectFilesystemError, ProjectInstanceId, ProjectSession};

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
    project_session_id: crate::project::ProjectSessionId,
    database_id: String,
    database_revision: u64,
}

impl ProjectState {
    pub(crate) fn reserve_database_operation(
        &self,
        project_instance_id: &crate::project::ProjectInstanceId,
        operation_id: crate::project::OperationId,
    ) -> Result<
        crate::project::resource_mutations::ResourceOperationReservation,
        ProjectDatabaseError,
    > {
        self.ensure_project_operational()?;
        self.reserve_resource_operation(project_instance_id, operation_id)
            .map_err(Into::into)
    }

    /// Runs a read/query operation against a detached database snapshot.
    #[cfg(test)]
    pub(crate) fn with_database_snapshot<F, R>(
        &self,
        id: &str,
        f: F,
    ) -> Result<R, ProjectDatabaseError>
    where
        F: FnOnce(&mut DatabaseInstance) -> Result<R, String>,
    {
        self.ensure_project_operational()?;
        let mut database = self
            .project_store
            .read()
            .unwrap()
            .databases
            .get(id)
            .cloned()
            .ok_or(ProjectDatabaseError::DatabaseNotFound)?;
        f(&mut database).map_err(ProjectDatabaseError::operation)
    }

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

    #[cfg(test)]
    pub(crate) fn with_database_snapshot_for_project<F, R>(
        &self,
        project_instance_id: &ProjectInstanceId,
        id: &str,
        f: F,
    ) -> Result<R, ProjectDatabaseError>
    where
        F: FnOnce(&mut DatabaseInstance) -> R,
    {
        let session = self.validate_database_project_identity(project_instance_id)?;
        let (_, mut database) = self.database_snapshot_for_session(&session, id)?;
        Ok(f(&mut database))
    }

    #[cfg(test)]
    pub fn with_database_writer<F, R>(
        &self,
        project_instance_id: &crate::project::ProjectInstanceId,
        id: &str,
        expected_revision: crate::project::ResourceRevision,
        operation_id: crate::project::OperationId,
        f: F,
    ) -> Result<
        crate::schema::application_event::ResourceMutationCommandResultDto<R>,
        ProjectDatabaseError,
    >
    where
        F: FnOnce(&mut DatabaseInstance, &ProjectSession) -> Result<R, String>,
    {
        let reservation = self.reserve_database_operation(project_instance_id, operation_id)?;
        let (session, _lease) = self.acquire_database_write_lease()?;
        let (token, mut instance) = self.revisioned_database_snapshot_for_session(
            &session,
            project_instance_id,
            id,
            expected_revision,
        )?;
        let data = f(&mut instance, &session).map_err(ProjectDatabaseError::operation)?;
        let mutation = self.commit_database_instance(&session, &token, instance, operation_id)?;
        let result =
            crate::schema::application_event::ResourceMutationCommandResultDto { data, mutation };
        reservation.complete();
        Ok(result)
    }

    #[cfg(test)]
    fn validate_database_project(
        session: &ProjectSession,
        project_instance_id: &crate::project::ProjectInstanceId,
    ) -> Result<(), ProjectDatabaseError> {
        if &session.instance_id != project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project instance changed".into(),
            }
            .into());
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn acquire_database_write_lease(
        &self,
    ) -> Result<(ProjectSession, ProjectFilesystemLeaseSet), ProjectDatabaseError> {
        self.ensure_project_operational()?;
        let session = self.capture_project_session()?;
        let lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_project_session(&session)?;
        Ok((session, lease))
    }

    #[cfg(test)]
    pub(crate) fn database_snapshot_for_session(
        &self,
        session: &ProjectSession,
        id: &str,
    ) -> Result<(DatabaseAuthorityToken, DatabaseInstance), ProjectDatabaseError> {
        self.validate_project_session(session)?;
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
        let instance = store
            .databases
            .get(id)
            .cloned()
            .ok_or(ProjectDatabaseError::DatabaseNotFound)?;
        let database_revision = revisions.get(id).copied().ok_or_else(|| {
            ProjectDatabaseError::from(ProjectFilesystemError::StaleProjectLifecycle {
                message: "database authority is missing".into(),
            })
        })?;
        Ok((
            DatabaseAuthorityToken {
                project_instance_id: publication.project_instance_id.clone(),
                project_session_id: store.project_session_id.clone(),
                database_id: id.to_string(),
                database_revision,
            },
            instance,
        ))
    }

    /// Captures only the durable declaration authority needed by the
    /// Application/Database two-owner mutation handoff. The physical
    /// `DatabaseInstance` never crosses this seam.
    pub(crate) fn prepare_database_mutation_authority(
        &self,
        project_instance_id: &ProjectInstanceId,
        id: &str,
        expected_revision: crate::project::ResourceRevision,
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
        operation_id: crate::project::OperationId,
    ) -> Result<crate::project::project_writers::ProjectResourceMutationFacts, ProjectDatabaseError>
    {
        let session = self.validate_database_project_identity(project_instance_id)?;
        self.commit_database_declaration(&session, token, after, operation_id)
    }

    #[cfg(test)]
    pub(crate) fn revisioned_database_snapshot_for_session(
        &self,
        session: &ProjectSession,
        project_instance_id: &crate::project::ProjectInstanceId,
        id: &str,
        expected_revision: crate::project::ResourceRevision,
    ) -> Result<(DatabaseAuthorityToken, DatabaseInstance), ProjectDatabaseError> {
        Self::validate_database_project(session, project_instance_id)?;
        let (token, instance) = self.database_snapshot_for_session(session, id)?;
        if token.database_revision != expected_revision.get() {
            return Err(ProjectDatabaseError::StaleDatabaseRevision);
        }
        Ok((token, instance))
    }

    fn validate_database_authority(
        publication: &super::project_state::MutationPublication,
        session: &ProjectSession,
        current_session_id: &crate::project::ProjectSessionId,
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
        retained: crate::project::ResourceRevision,
    ) -> Result<crate::project::ResourceRevision, ProjectDatabaseError> {
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
        from_revision: crate::project::ResourceRevision,
        to_revision: crate::project::ResourceRevision,
        operation_id: crate::project::OperationId,
        publication_advance: super::project_state::PreparedPublicationAdvance,
        before: Option<DatabaseDecl>,
        after: Option<DatabaseDecl>,
    ) -> crate::project::project_writers::ProjectResourceMutationFacts {
        use crate::project::history::{
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

    #[cfg(test)]
    pub(crate) fn commit_database_instance(
        &self,
        session: &ProjectSession,
        token: &DatabaseAuthorityToken,
        instance: DatabaseInstance,
        operation_id: crate::project::OperationId,
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, ProjectDatabaseError>
    {
        let id = token.database_id.as_str();
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        let mut store = self.project_store.write().unwrap();
        let mut revisions = self.database_authority_revisions.write().unwrap();
        Self::validate_database_authority(
            &publication,
            session,
            &store.project_session_id,
            &revisions,
            token,
            id,
        )?;
        let before = data
            .databases
            .get(id)
            .cloned()
            .ok_or(ProjectDatabaseError::DatabaseNotFound)?;
        if !store.databases.contains_key(id) {
            return Err(ProjectDatabaseError::DatabaseNotFound);
        }
        let publication_advance = publication.prepare_resource_revision()?;
        let from_revision = crate::project::ResourceRevision::new(token.database_revision);
        let to_revision = Self::next_database_revision(id, from_revision)?;
        let after = instance.decl.clone();
        data.databases.insert(id.to_string(), after.clone());
        store.databases.insert(id.to_string(), instance);
        let mutation = self.publish_database_delta(
            &mut publication,
            &mut revisions,
            id,
            from_revision,
            to_revision,
            operation_id,
            publication_advance,
            Some(before),
            Some(after),
        );
        Ok(mutation.into_transport())
    }

    pub(crate) fn commit_database_declaration(
        &self,
        session: &ProjectSession,
        token: DatabaseAuthorityToken,
        after: DatabaseDecl,
        operation_id: crate::project::OperationId,
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
        let from_revision = crate::project::ResourceRevision::new(token.database_revision);
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
        operation_id: crate::project::OperationId,
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
        let from_revision = crate::project::ResourceRevision::INITIAL;
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
        expected_revision: crate::project::ResourceRevision,
        operation_id: crate::project::OperationId,
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
        let from_revision = crate::project::ResourceRevision::new(current_revision);
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

    #[cfg(test)]
    pub(crate) fn add_database_for_session(
        &self,
        session: &ProjectSession,
        project_instance_id: &crate::project::ProjectInstanceId,
        operation_id: crate::project::OperationId,
        instance: DatabaseInstance,
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, ProjectDatabaseError>
    {
        let reservation = self.reserve_database_operation(project_instance_id, operation_id)?;
        let mutation = self.commit_database_add_for_session(
            session,
            project_instance_id,
            operation_id,
            instance,
        )?;
        reservation.complete();
        Ok(mutation)
    }

    #[cfg(test)]
    pub(crate) fn commit_database_add_for_session(
        &self,
        session: &ProjectSession,
        project_instance_id: &crate::project::ProjectInstanceId,
        operation_id: crate::project::OperationId,
        instance: DatabaseInstance,
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, ProjectDatabaseError>
    {
        Self::validate_database_project(session, project_instance_id)?;
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project instance changed".into(),
            }
            .into());
        }
        let decl = instance.decl.clone();
        let id = decl.id.as_str().to_string();
        let mut data = self.project_data.write().unwrap();
        let mut store = self.project_store.write().unwrap();
        let mut revisions = self.database_authority_revisions.write().unwrap();
        if data.databases.contains_key(&id) || store.databases.contains_key(&id) {
            return Err(ProjectDatabaseError::DatabaseAlreadyExists);
        }
        let publication_advance = publication.prepare_resource_revision()?;
        let from_revision = revisions
            .get(&id)
            .copied()
            .map(crate::project::ResourceRevision::new)
            .unwrap_or(crate::project::ResourceRevision::INITIAL);
        let to_revision = Self::next_database_revision(&id, from_revision)?;
        data.databases.insert(id.clone(), decl.clone());
        store.databases.insert(id.clone(), instance);
        let mutation = self.publish_database_delta(
            &mut publication,
            &mut revisions,
            &id,
            from_revision,
            to_revision,
            operation_id,
            publication_advance,
            None,
            Some(decl),
        );
        Ok(mutation.into_transport())
    }

    #[cfg(test)]
    pub(crate) fn commit_database_name(
        &self,
        session: &ProjectSession,
        token: &DatabaseAuthorityToken,
        id: &str,
        name: &str,
        operation_id: crate::project::OperationId,
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, ProjectDatabaseError>
    {
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        let mut store = self.project_store.write().unwrap();
        let mut revisions = self.database_authority_revisions.write().unwrap();
        Self::validate_database_authority(
            &publication,
            session,
            &store.project_session_id,
            &revisions,
            token,
            id,
        )?;
        let declaration = data
            .databases
            .get_mut(id)
            .ok_or(ProjectDatabaseError::DatabaseNotFound)?;
        let before = declaration.clone();
        let instance = store
            .databases
            .get_mut(id)
            .ok_or(ProjectDatabaseError::DatabaseNotFound)?;
        let publication_advance = publication.prepare_resource_revision()?;
        let from_revision = crate::project::ResourceRevision::new(token.database_revision);
        let to_revision = Self::next_database_revision(id, from_revision)?;
        declaration.name = name.to_string().into();
        instance.decl.name = name.to_string().into();
        let after = declaration.clone();
        let mutation = self.publish_database_delta(
            &mut publication,
            &mut revisions,
            id,
            from_revision,
            to_revision,
            operation_id,
            publication_advance,
            Some(before),
            Some(after),
        );
        Ok(mutation.into_transport())
    }

    #[cfg(test)]
    pub fn delete_database(
        &self,
        project_instance_id: &crate::project::ProjectInstanceId,
        id: &str,
        expected_revision: crate::project::ResourceRevision,
        operation_id: crate::project::OperationId,
    ) -> Result<
        crate::schema::application_event::ResourceMutationCommandResultDto<()>,
        ProjectDatabaseError,
    > {
        let reservation = self.reserve_database_operation(project_instance_id, operation_id)?;
        let (session, _lease) = self.acquire_database_write_lease()?;
        let (token, instance) = self.revisioned_database_snapshot_for_session(
            &session,
            project_instance_id,
            id,
            expected_revision,
        )?;
        crate::database::remove_duckdb_table_if_needed(
            &instance.decl.engine,
            Some(session.root.as_path()),
        )
        .map_err(ProjectDatabaseError::operation)?;
        let mutation = self.commit_database_delete(&session, &token, id, operation_id)?;
        let result = crate::schema::application_event::ResourceMutationCommandResultDto {
            data: (),
            mutation,
        };
        reservation.complete();
        Ok(result)
    }

    #[cfg(test)]
    pub(crate) fn commit_database_delete(
        &self,
        session: &ProjectSession,
        token: &DatabaseAuthorityToken,
        id: &str,
        operation_id: crate::project::OperationId,
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, ProjectDatabaseError>
    {
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        let mut store = self.project_store.write().unwrap();
        let mut revisions = self.database_authority_revisions.write().unwrap();
        Self::validate_database_authority(
            &publication,
            session,
            &store.project_session_id,
            &revisions,
            token,
            id,
        )?;
        let publication_advance = publication.prepare_resource_revision()?;
        let from_revision = crate::project::ResourceRevision::new(token.database_revision);
        let to_revision = Self::next_database_revision(id, from_revision)?;
        let before = data
            .databases
            .remove(id)
            .ok_or(ProjectDatabaseError::DatabaseNotFound)?;
        if store.databases.remove(id).is_none() {
            data.databases.insert(id.to_string(), before);
            return Err(ProjectDatabaseError::DatabaseNotFound);
        }
        let mutation = self.publish_database_delta(
            &mut publication,
            &mut revisions,
            id,
            from_revision,
            to_revision,
            operation_id,
            publication_advance,
            Some(before),
            None,
        );
        Ok(mutation.into_transport())
    }
}

#[cfg(all(test, any()))]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn project_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "yssbi-database-writer-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut data = crate::project::ProjectData::new();
        data.metadata.project_name = label.into();
        crate::project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        root
    }

    fn database_basis(
        state: &ProjectState,
        id: &str,
    ) -> (
        crate::project::ProjectInstanceId,
        crate::project::ResourceRevision,
        crate::project::OperationId,
    ) {
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let revision = state
            .database_authority_revisions
            .read()
            .unwrap()
            .get(id)
            .copied()
            .map(crate::project::ResourceRevision::new)
            .unwrap_or(crate::project::ResourceRevision::INITIAL);
        (
            project_instance_id,
            revision,
            crate::project::OperationId::new(),
        )
    }

    fn add_database_fixture(
        state: &ProjectState,
        instance: DatabaseInstance,
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, ProjectDatabaseError>
    {
        let (session, _lease) = state.acquire_database_write_lease()?;
        state.add_database_for_session(
            &session,
            &session.instance_id,
            crate::project::OperationId::new(),
            instance,
        )
    }

    fn load_database_fixture(
        state: &ProjectState,
        engine: crate::schema::DatabaseEngineDTO,
    ) -> Result<crate::application::database::LoadDatabaseResult, ProjectDatabaseError> {
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        crate::application::database::load_database(
            state,
            &project_instance_id,
            crate::project::OperationId::new(),
            engine,
        )
        .map(|result| result.data)
    }

    #[test]
    fn database_reads_reject_stale_project_identity() {
        let state = ProjectState::new();
        let mut original = crate::project::ProjectData::new();
        original.databases.insert(
            "sales".into(),
            DatabaseDecl {
                id: DatabaseId::from_existing("sales".into()),
                engine: crate::database_contract::DatabaseEngine::InMemory {
                    name: "sales".into(),
                },
                schema_version: 1,
                required: false,
                name: "Original".into(),
            },
        );
        state.activate_project_fixture("database-read-original".into(), original);
        let stale = state.capture_project_session().unwrap().instance_id;
        let mut replacement = crate::project::ProjectData::new();
        replacement.databases.insert(
            "sales".into(),
            DatabaseDecl {
                id: DatabaseId::from_existing("sales".into()),
                engine: crate::database_contract::DatabaseEngine::InMemory {
                    name: "sales".into(),
                },
                schema_version: 1,
                required: false,
                name: "Replacement".into(),
            },
        );
        state.activate_project_fixture("database-read-replacement".into(), replacement);
        let closure_called = std::sync::atomic::AtomicBool::new(false);

        let result = state.with_database_snapshot_for_project(&stale, "sales", |_| {
            closure_called.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        assert_eq!(
            result.unwrap_err().command_code(),
            Some("stale_project_lifecycle")
        );
        assert!(!closure_called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn database_mutation_commits_authority_and_errors_have_zero_effects() {
        let root = project_root("authoritative-mutation");
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        let database_id = "writer";
        add_database_fixture(
            &state,
            DatabaseInstance {
                decl: DatabaseDecl {
                    id: DatabaseId::from_existing(database_id.into()),
                    engine: crate::database_contract::DatabaseEngine::InMemory {
                        name: "writer".into(),
                    },
                    schema_version: 1,
                    required: false,
                    name: "Before".into(),
                },
                state: DatabaseState::Failed {
                    error: "fixture".into(),
                },
            },
        )
        .unwrap();
        let graph_path =
            crate::graph_document::GraphResourcePath::new("events/DatabaseWriter.yssbi-event")
                .unwrap();
        state
            .insert_graph(
                graph_path.clone(),
                crate::project::GraphResourceDocument::new(
                    "Database Writer",
                    crate::project::GraphDocumentKind::Event,
                ),
            )
            .unwrap();
        state.graph_projection(&graph_path, "en-US").unwrap();
        let coordinator = state.compile_coordinator.read().unwrap().clone();
        let document_path = graph_path.clone();
        assert!(coordinator.contains_slot_for_test(&document_path));
        let generation = state.authority_generation_for_test();

        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, database_id);
        let closure_had_no_store_lock = state
            .with_database_writer(
                &project_instance_id,
                database_id,
                expected_revision,
                operation_id,
                |database, _| {
                    let store_lock = state.project_store.try_write();
                    let lock_available = store_lock.is_ok();
                    drop(store_lock);
                    database.decl.name = "Committed".into();
                    Ok(lock_available)
                },
            )
            .unwrap()
            .data;

        assert!(closure_had_no_store_lock);
        assert_eq!(
            state.project_store.read().unwrap().databases[database_id]
                .decl
                .name
                .as_ref(),
            "Committed"
        );
        assert_eq!(
            state.get_data().unwrap().databases[database_id]
                .name
                .as_ref(),
            "Committed"
        );
        assert_eq!(state.authority_generation_for_test(), generation + 1);
        assert!(coordinator.contains_slot_for_test(&document_path));

        state.graph_projection(&graph_path, "en-US").unwrap();
        let generation = state.authority_generation_for_test();
        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, database_id);
        let error = state
            .with_database_writer(
                &project_instance_id,
                database_id,
                expected_revision,
                operation_id,
                |database, _| {
                    database.decl.name = "Rejected".into();
                    Err::<(), _>("reject mutation".into())
                },
            )
            .unwrap_err();
        assert!(matches!(
            error,
            ProjectDatabaseError::Operation(message) if message == "reject mutation"
        ));
        assert_eq!(
            state.project_store.read().unwrap().databases[database_id]
                .decl
                .name
                .as_ref(),
            "Committed"
        );
        assert_eq!(state.authority_generation_for_test(), generation);
        assert!(coordinator.contains_slot_for_test(&document_path));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn database_snapshot_access_cannot_mutate_authority_or_invalidate_compiles() {
        let root = project_root("snapshot-access");
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        let database_id = "snapshot";
        add_database_fixture(
            &state,
            DatabaseInstance {
                decl: DatabaseDecl {
                    id: DatabaseId::from_existing(database_id.into()),
                    engine: crate::database_contract::DatabaseEngine::InMemory {
                        name: "snapshot".into(),
                    },
                    schema_version: 1,
                    required: false,
                    name: "Authoritative".into(),
                },
                state: DatabaseState::Failed {
                    error: "fixture".into(),
                },
            },
        )
        .unwrap();
        let graph_path =
            crate::graph_document::GraphResourcePath::new("events/DatabaseSnapshot.yssbi-event")
                .unwrap();
        state
            .insert_graph(
                graph_path.clone(),
                crate::project::GraphResourceDocument::new(
                    "Database Snapshot",
                    crate::project::GraphDocumentKind::Event,
                ),
            )
            .unwrap();
        state.graph_projection(&graph_path, "en-US").unwrap();
        let coordinator = state.compile_coordinator.read().unwrap().clone();
        let document_path = graph_path.clone();
        assert!(coordinator.contains_slot_for_test(&document_path));
        let generation = state.authority_generation_for_test();

        let closure_had_no_store_lock = state
            .with_database_snapshot(database_id, |database| {
                let store_lock = state.project_store.try_write();
                let lock_available = store_lock.is_ok();
                drop(store_lock);
                database.decl.name = "Local only".into();
                Ok(lock_available)
            })
            .unwrap();
        state
            .with_database_snapshot(database_id, |database| {
                assert_eq!(database.decl.name.as_ref(), "Authoritative");
                Ok(())
            })
            .unwrap();

        assert!(closure_had_no_store_lock);
        assert_eq!(
            state.project_store.read().unwrap().databases[database_id]
                .decl
                .name
                .as_ref(),
            "Authoritative"
        );
        assert_eq!(state.authority_generation_for_test(), generation);
        assert!(coordinator.contains_slot_for_test(&document_path));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn duckdb_import_rename_edit_save_and_delete_advance_central_authority() {
        let root = project_root("central-writers");
        let csv = root.join("writer.csv");
        std::fs::write(&csv, "value\n1\n").unwrap();
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        let initial = state.authority_generation_for_test();
        let project_instance_id = state.capture_project_session().unwrap().instance_id;

        let imported_result = crate::application::database::load_database(
            &state,
            &project_instance_id,
            crate::project::OperationId::new(),
            crate::schema::DatabaseEngineDTO::Csv {
                path: csv.to_string_lossy().into_owned(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: None,
            },
        )
        .unwrap();
        assert_eq!(imported_result.mutation.publication_revision, 1);
        let imported = imported_result.data;
        let after_import = state.authority_generation_for_test();
        assert_eq!(after_import, initial + 1);
        assert_eq!(
            state.database_authority_revisions.read().unwrap()[&imported.id],
            imported_result.mutation.deltas[0].to_revision.get()
        );

        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, &imported.id);
        let renamed = crate::application::database::rename_database(
            &state,
            &project_instance_id,
            &imported.id,
            expected_revision,
            "renamed",
            operation_id,
        )
        .unwrap();
        assert_eq!(renamed.mutation.publication_revision, 2);
        let after_rename = state.authority_generation_for_test();
        assert_eq!(after_rename, after_import + 1);
        assert_eq!(
            state.database_authority_revisions.read().unwrap()[&imported.id],
            renamed.mutation.deltas[0].to_revision.get()
        );

        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, &imported.id);
        let edited = state
            .with_database_writer(
                &project_instance_id,
                &imported.id,
                expected_revision,
                operation_id,
                |database, _| database.add_column("added", "Int64"),
            )
            .unwrap();
        assert_eq!(edited.mutation.publication_revision, 3);
        let after_edit = state.authority_generation_for_test();
        assert_eq!(after_edit, after_rename + 1);
        assert_eq!(
            state.database_authority_revisions.read().unwrap()[&imported.id],
            edited.mutation.deltas[0].to_revision.get()
        );
        let metadata = crate::application::database::read_database_meta(
            &state,
            &project_instance_id,
            &imported.id,
        )
        .unwrap();
        assert!(metadata.columns.iter().any(|column| column.name == "added"));

        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, &imported.id);
        let saved = crate::application::database::save_database_changes(
            &state,
            &project_instance_id,
            &imported.id,
            expected_revision,
            operation_id,
        )
        .unwrap();
        assert_eq!(saved.mutation.publication_revision, 4);
        let after_save = state.authority_generation_for_test();
        assert_eq!(after_save, after_edit + 1);
        assert_eq!(
            state.database_authority_revisions.read().unwrap()[&imported.id],
            saved.mutation.deltas[0].to_revision.get()
        );

        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, &imported.id);
        let deleted = state
            .delete_database(
                &project_instance_id,
                &imported.id,
                expected_revision,
                operation_id,
            )
            .unwrap();
        assert_eq!(deleted.mutation.publication_revision, 5);
        assert_eq!(state.authority_generation_for_test(), after_save + 1);
        assert!(
            !state
                .get_data()
                .unwrap()
                .databases
                .contains_key(&imported.id)
        );
        assert!(
            !state
                .project_store
                .read()
                .unwrap()
                .databases
                .contains_key(&imported.id)
        );
        assert!(
            !crate::project::load_project_from_file(root.to_string_lossy().as_ref())
                .unwrap()
                .databases
                .contains_key(&imported.id)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn add_in_memory_database(state: &ProjectState, id: &str, name: &str) {
        add_database_fixture(
            state,
            DatabaseInstance {
                decl: DatabaseDecl {
                    id: DatabaseId::from_existing(id.into()),
                    engine: crate::database_contract::DatabaseEngine::InMemory { name: id.into() },
                    schema_version: 1,
                    required: false,
                    name: name.into(),
                },
                state: DatabaseState::Failed {
                    error: "fixture".into(),
                },
            },
        )
        .unwrap();
    }

    #[test]
    fn unrelated_authority_change_does_not_conflict_database_commit() {
        let root = project_root("resource-cas-unrelated");
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        add_in_memory_database(&state, "writer", "Initial");
        let session = state.capture_project_session().unwrap();
        let (token, _) = state
            .database_snapshot_for_session(&session, "writer")
            .unwrap();

        state
            .insert_graph(
                crate::graph_document::GraphResourcePath::new("events/Unrelated.yssbi-event")
                    .unwrap(),
                crate::project::GraphResourceDocument::new(
                    "Unrelated",
                    crate::project::GraphDocumentKind::Event,
                ),
            )
            .unwrap();
        let generation_after_unrelated = state.authority_generation_for_test();

        state
            .commit_database_name(
                &session,
                &token,
                "writer",
                "Committed",
                crate::project::OperationId::new(),
            )
            .unwrap();

        assert_eq!(
            state.project_store.read().unwrap().databases["writer"]
                .decl
                .name
                .as_ref(),
            "Committed"
        );
        assert_eq!(
            state.authority_generation_for_test(),
            generation_after_unrelated + 1
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rename_remains_consistent_across_unrelated_authority_change_after_io() {
        let root = project_root("rename-unrelated-authority");
        let csv = root.join("rename.csv");
        std::fs::write(&csv, "value\n1\n").unwrap();
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        let imported = load_database_fixture(
            &state,
            crate::schema::DatabaseEngineDTO::Csv {
                path: csv.to_string_lossy().into_owned(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: None,
            },
        )
        .unwrap();
        let (duckdb_path, table) = {
            let store = state.project_store.read().unwrap();
            let database = &store.databases[&imported.id];
            let DatabaseState::DuckDb {
                duckdb_path, table, ..
            } = &database.state
            else {
                panic!("imported database should be DuckDB-backed")
            };
            (std::path::PathBuf::from(duckdb_path), table.clone())
        };
        let (io_done_tx, io_done_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let resume_rx = std::sync::Mutex::new(resume_rx);
        crate::application::database::set_database_external_io_test_hook(Some(Arc::new(
            move || {
                io_done_tx.send(()).unwrap();
                resume_rx.lock().unwrap().recv().unwrap();
            },
        )));

        let rename_state = state.clone();
        let rename_id = imported.id.clone();
        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, &rename_id);
        let rename = std::thread::spawn(move || {
            crate::application::database::rename_database(
                &rename_state,
                &project_instance_id,
                &rename_id,
                expected_revision,
                "Renamed",
                operation_id,
            )
        });
        io_done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        state
            .insert_graph(
                crate::graph_document::GraphResourcePath::new("events/UnrelatedRename.yssbi-event")
                    .unwrap(),
                crate::project::GraphResourceDocument::new(
                    "Unrelated Rename",
                    crate::project::GraphDocumentKind::Event,
                ),
            )
            .unwrap();
        resume_tx.send(()).unwrap();
        let rename = rename.join().unwrap();
        crate::application::database::set_database_external_io_test_hook(None);

        rename.unwrap();
        assert_eq!(
            state.project_store.read().unwrap().databases[&imported.id]
                .decl
                .name
                .as_ref(),
            "Renamed"
        );
        assert_eq!(
            crate::database::read_display_name(&duckdb_path, &table).as_deref(),
            Some("Renamed")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stale_database_writer_cannot_overwrite_newer_authority() {
        let root = project_root("writer-cas");
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        add_in_memory_database(&state, "writer", "Initial");
        let session = state.capture_project_session().unwrap();
        let (snapshot_ready_tx, snapshot_ready_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();

        let stale_state = state.clone();
        let stale_session = session.clone();
        let stale = std::thread::spawn(move || {
            let (token, mut instance) = stale_state
                .database_snapshot_for_session(&stale_session, "writer")
                .unwrap();
            snapshot_ready_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
            instance.decl.name = "Stale A".into();
            stale_state.commit_database_instance(
                &stale_session,
                &token,
                instance,
                crate::project::OperationId::new(),
            )
        });
        snapshot_ready_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, "writer");
        state
            .with_database_writer(
                &project_instance_id,
                "writer",
                expected_revision,
                operation_id,
                |database, _| {
                    database.decl.name = "Committed B".into();
                    Ok(())
                },
            )
            .unwrap();
        let generation_after_b = state.authority_generation_for_test();
        resume_tx.send(()).unwrap();

        let error = stale.join().unwrap().unwrap_err();
        assert_eq!(error.command_code(), Some("stale_project_lifecycle"));
        assert_eq!(state.authority_generation_for_test(), generation_after_b);
        assert_eq!(
            state.project_store.read().unwrap().databases["writer"]
                .decl
                .name
                .as_ref(),
            "Committed B"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn database_operation_ids_reject_in_flight_and_completed_replays_and_release_failures() {
        use crate::project::{OperationId, ResourceRevision};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        let root = project_root("database-operation-ledger");
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        add_in_memory_database(&state, "writer", "Initial");
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let expected_revision =
            ResourceRevision::new(state.database_authority_revisions.read().unwrap()["writer"]);
        let operation_id = OperationId::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();

        let first_state = state.clone();
        let first_project = project_instance_id.clone();
        let first_calls = Arc::clone(&calls);
        let first = std::thread::spawn(move || {
            first_state.with_database_writer(
                &first_project,
                "writer",
                expected_revision,
                operation_id,
                |database, _| {
                    first_calls.fetch_add(1, Ordering::SeqCst);
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    database.decl.name = "First".into();
                    Ok(())
                },
            )
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let duplicate_state = state.clone();
        let duplicate_project = project_instance_id.clone();
        let duplicate_calls = Arc::clone(&calls);
        let (duplicate_tx, duplicate_rx) = std::sync::mpsc::channel();
        let duplicate = std::thread::spawn(move || {
            let result = duplicate_state.with_database_writer(
                &duplicate_project,
                "writer",
                expected_revision,
                operation_id,
                |_, _| {
                    duplicate_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            );
            let _ = duplicate_tx.send(result);
        });
        let in_flight = match duplicate_rx.recv_timeout(Duration::from_millis(250)) {
            Ok(result) => result,
            Err(error) => {
                release_tx.send(()).unwrap();
                first.join().unwrap().unwrap();
                duplicate.join().unwrap();
                panic!("duplicate operation did not reject before waiting for the writer: {error}");
            }
        };
        assert_eq!(
            in_flight.unwrap_err().command_code(),
            Some("duplicate_operation")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        release_tx.send(()).unwrap();
        let first = first.join().unwrap().unwrap();
        duplicate.join().unwrap();
        let committed_revision = first.mutation.deltas[0].to_revision;
        let replay = state.with_database_writer(
            &project_instance_id,
            "writer",
            committed_revision,
            operation_id,
            |_, _| Ok(()),
        );
        assert_eq!(
            replay.unwrap_err().command_code(),
            Some("duplicate_operation")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let retryable_operation = OperationId::new();
        let failed = state.with_database_writer(
            &project_instance_id,
            "writer",
            committed_revision,
            retryable_operation,
            |_, _| Err::<(), _>("injected failure".into()),
        );
        assert!(matches!(
            failed.unwrap_err(),
            ProjectDatabaseError::Operation(message) if message == "injected failure"
        ));
        let retried = state
            .with_database_writer(
                &project_instance_id,
                "writer",
                committed_revision,
                retryable_operation,
                |database, _| {
                    database.decl.required = true;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(retried.mutation.operation_id, retryable_operation);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn discovery_database_commits_publish_once_and_failures_have_zero_effects() {
        use crate::graph::document::{DatabaseResourceKey, ResourceDocumentPatch, ResourceKey};
        use crate::project::{OperationId, ResourceRevision};

        let root = project_root("canonical-publication");
        let state = ProjectState::new();
        let project = state.activate_project_from_path(&root).unwrap();
        let (session, _lease) = state.acquire_database_write_lease().unwrap();
        let database_id = "writer";
        let operation_id = OperationId::new();
        let created = state
            .add_database_for_session(
                &session,
                &project.instance_id,
                operation_id,
                DatabaseInstance {
                    decl: DatabaseDecl {
                        id: DatabaseId::from_existing(database_id.into()),
                        engine: crate::database_contract::DatabaseEngine::InMemory {
                            name: database_id.into(),
                        },
                        schema_version: 1,
                        required: false,
                        name: "Before".into(),
                    },
                    state: DatabaseState::Failed {
                        error: "fixture".into(),
                    },
                },
            )
            .unwrap();
        drop(_lease);

        assert_eq!(created.publication_revision, 1);
        assert_eq!(created.operation_id, operation_id);
        assert_eq!(created.project_instance_id, project.instance_id.as_str());
        assert_eq!(created.deltas.len(), 1);
        assert_eq!(
            created.deltas[0].resource,
            ResourceKey::Database(DatabaseResourceKey("databases/writer".into()))
        );
        assert_eq!(created.deltas[0].from_revision, ResourceRevision::INITIAL);
        assert_eq!(created.deltas[0].caused_by, Some(operation_id));
        assert!(matches!(
            created.deltas[0].payload,
            ResourceDocumentPatch::Database(_)
        ));

        let created_revision =
            ResourceRevision::new(state.database_authority_revisions.read().unwrap()[database_id]);
        assert_eq!(created.deltas[0].to_revision, created_revision);

        let publication_before_duplicate =
            state.mutation_publication.lock().unwrap().resource_revision;
        let authority_before_duplicate = state.database_authority_revisions.read().unwrap().clone();
        let data_before_duplicate = state.get_data().unwrap().databases[database_id].clone();
        let duplicate = add_database_fixture(
            &state,
            DatabaseInstance {
                decl: data_before_duplicate.clone(),
                state: DatabaseState::Failed {
                    error: "duplicate fixture".into(),
                },
            },
        );
        assert!(matches!(
            duplicate.unwrap_err(),
            ProjectDatabaseError::DatabaseAlreadyExists
        ));
        assert_eq!(
            state.mutation_publication.lock().unwrap().resource_revision,
            publication_before_duplicate
        );
        assert_eq!(
            *state.database_authority_revisions.read().unwrap(),
            authority_before_duplicate
        );
        assert_eq!(
            state.get_data().unwrap().databases[database_id],
            data_before_duplicate
        );

        let edit_operation = OperationId::new();
        let edited = state
            .with_database_writer(
                &project.instance_id,
                database_id,
                created_revision,
                edit_operation,
                |database, _| {
                    database.decl.schema_version = 2;
                    Ok("edited")
                },
            )
            .unwrap();
        assert_eq!(edited.data, "edited");
        assert_eq!(edited.mutation.publication_revision, 2);
        assert_eq!(edited.mutation.operation_id, edit_operation);
        assert_eq!(edited.mutation.deltas[0].from_revision, created_revision);
        let edited_revision = edited.mutation.deltas[0].to_revision;
        assert_eq!(
            state.database_authority_revisions.read().unwrap()[database_id],
            edited_revision.get()
        );

        let revision_before_failure = state.mutation_publication.lock().unwrap().resource_revision;
        let authority_before_failure = state.database_authority_revisions.read().unwrap().clone();
        let data_before_failure = state.get_data().unwrap().databases[database_id].clone();
        let failure = state.with_database_writer(
            &project.instance_id,
            database_id,
            edited_revision,
            OperationId::new(),
            |database, _| {
                database.decl.name = "Rejected".into();
                Err::<(), _>("reject mutation".into())
            },
        );
        assert!(matches!(
            failure.unwrap_err(),
            ProjectDatabaseError::Operation(message) if message == "reject mutation"
        ));
        assert_eq!(
            state.mutation_publication.lock().unwrap().resource_revision,
            revision_before_failure
        );
        assert_eq!(
            *state.database_authority_revisions.read().unwrap(),
            authority_before_failure
        );
        assert_eq!(
            state.get_data().unwrap().databases[database_id].name,
            data_before_failure.name
        );

        let stale = state.with_database_writer(
            &project.instance_id,
            database_id,
            created_revision,
            OperationId::new(),
            |database, _| {
                database.decl.required = true;
                Ok(())
            },
        );
        assert!(matches!(
            stale.unwrap_err(),
            ProjectDatabaseError::StaleDatabaseRevision
        ));
        assert_eq!(
            state.mutation_publication.lock().unwrap().resource_revision,
            revision_before_failure
        );
        assert_eq!(
            *state.database_authority_revisions.read().unwrap(),
            authority_before_failure
        );

        let delete_operation = OperationId::new();
        let deleted = state
            .delete_database(
                &project.instance_id,
                database_id,
                edited_revision,
                delete_operation,
            )
            .unwrap();
        assert_eq!(deleted.mutation.publication_revision, 3);
        assert_eq!(deleted.mutation.operation_id, delete_operation);
        assert_eq!(deleted.mutation.deltas[0].from_revision, edited_revision);
        assert_eq!(
            state.database_authority_revisions.read().unwrap()[database_id],
            edited_revision.next().get(),
            "deletion must retain a non-reusable database tombstone",
        );

        let mut replacement = crate::project::ProjectData::new();
        replacement.databases.insert(
            "replacement".into(),
            DatabaseDecl {
                id: DatabaseId::from_existing("replacement".into()),
                engine: crate::database_contract::DatabaseEngine::InMemory {
                    name: "replacement".into(),
                },
                schema_version: 1,
                required: false,
                name: "Replacement".into(),
            },
        );
        state.activate_project_fixture("replacement-project".into(), replacement);
        assert_eq!(
            *state.database_authority_revisions.read().unwrap(),
            std::collections::HashMap::from([("replacement".into(), 0)])
        );
        assert_eq!(
            state.mutation_publication.lock().unwrap().resource_revision,
            0
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn database_rename_and_delete_share_authority_token_cas() {
        let root = project_root("rename-delete-cas");
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        add_in_memory_database(&state, "writer", "Initial");
        let session = state.capture_project_session().unwrap();

        let (rename_token, _) = state
            .database_snapshot_for_session(&session, "writer")
            .unwrap();
        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, "writer");
        state
            .with_database_writer(
                &project_instance_id,
                "writer",
                expected_revision,
                operation_id,
                |database, _| {
                    database.decl.name = "New Authority".into();
                    Ok(())
                },
            )
            .unwrap();
        let generation = state.authority_generation_for_test();
        assert_eq!(
            state
                .commit_database_name(
                    &session,
                    &rename_token,
                    "writer",
                    "Stale Rename",
                    crate::project::OperationId::new(),
                )
                .unwrap_err()
                .command_code(),
            Some("stale_project_lifecycle")
        );
        assert_eq!(state.authority_generation_for_test(), generation);

        let (delete_token, _) = state
            .database_snapshot_for_session(&session, "writer")
            .unwrap();
        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, "writer");
        state
            .with_database_writer(
                &project_instance_id,
                "writer",
                expected_revision,
                operation_id,
                |database, _| {
                    database.decl.required = true;
                    Ok(())
                },
            )
            .unwrap();
        let generation = state.authority_generation_for_test();
        assert_eq!(
            state
                .commit_database_delete(
                    &session,
                    &delete_token,
                    "writer",
                    crate::project::OperationId::new(),
                )
                .unwrap_err()
                .command_code(),
            Some("stale_project_lifecycle")
        );
        assert_eq!(state.authority_generation_for_test(), generation);
        assert!(
            state
                .project_store
                .read()
                .unwrap()
                .databases
                .contains_key("writer")
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
