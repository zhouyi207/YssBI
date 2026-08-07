use super::ProjectState;
use crate::database::*;
use crate::project::{
    ProjectFilesystemError, ProjectFilesystemLeaseSet, ProjectInstanceId, ProjectSession,
};
use polars::prelude::*;

#[derive(Clone)]
pub(crate) struct DatabaseAuthorityToken {
    project_instance_id: String,
    project_session_id: crate::node_system::analysis::ProjectSessionId,
    database_id: String,
    database_revision: u64,
}

/// let preview = project_state
///    .access_database("sales", DatabaseAccess::Preview)?;
impl ProjectState {
    pub fn access_database(&self, id: &str, access: DatabaseAccess) -> PolarsResult<DatabaseView> {
        self.ensure_project_operational().map_err(|error| {
            PolarsError::ComputeError(format!("{}: {error}", error.code()).into())
        })?;
        let mut database = self
            .project_store
            .read()
            .unwrap()
            .databases
            .get(id)
            .cloned()
            .ok_or_else(|| PolarsError::NoData("nodata".into()))?;

        database.access(access)
    }

    pub(crate) fn reserve_database_operation(
        &self,
        project_instance_id: &crate::project::ProjectInstanceId,
        operation_id: crate::node_system::document::OperationId,
    ) -> Result<crate::project::resource_mutations::ResourceOperationReservation, String> {
        self.ensure_project_operational()
            .map_err(|error| format!("{}: {error}", error.code()))?;
        self.reserve_resource_operation(project_instance_id, operation_id)
            .map_err(|error| format!("{}: {error}", error.code()))
    }

    /// Runs and commits an authoritative database mutation against an exact
    /// caller-issued project and database revision.
    pub fn with_database_mut<F, R>(
        &self,
        project_instance_id: &crate::project::ProjectInstanceId,
        id: &str,
        expected_revision: crate::node_system::document::ResourceRevision,
        operation_id: crate::node_system::document::OperationId,
        f: F,
    ) -> Result<crate::event::ResourceMutationCommandResultDto<R>, String>
    where
        F: FnOnce(&mut DatabaseInstance) -> Result<R, String>,
    {
        self.with_database_writer(
            project_instance_id,
            id,
            expected_revision,
            operation_id,
            |database, _| f(database),
        )
    }

    /// Runs a read/query operation against a detached database snapshot.
    pub(crate) fn with_database_snapshot<F, R>(&self, id: &str, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut DatabaseInstance) -> Result<R, String>,
    {
        self.ensure_project_operational()
            .map_err(|error| format!("{}: {error}", error.code()))?;
        let mut database = self
            .project_store
            .read()
            .unwrap()
            .databases
            .get(id)
            .cloned()
            .ok_or_else(|| "Database not found".to_string())?;
        f(&mut database)
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

    pub(crate) fn with_database_snapshot_for_project<F, R>(
        &self,
        project_instance_id: &ProjectInstanceId,
        id: &str,
        f: F,
    ) -> Result<R, ProjectFilesystemError>
    where
        F: FnOnce(&mut DatabaseInstance) -> R,
    {
        let session = self.validate_database_project_identity(project_instance_id)?;
        let (_, mut database) =
            self.database_snapshot_for_session(&session, id)
                .map_err(|error| {
                    if error.starts_with("stale_project_lifecycle:") {
                        ProjectFilesystemError::StaleProjectLifecycle { message: error }
                    } else {
                        ProjectFilesystemError::DatabaseAccessFailed { message: error }
                    }
                })?;
        Ok(f(&mut database))
    }

    pub fn with_database_writer<F, R>(
        &self,
        project_instance_id: &crate::project::ProjectInstanceId,
        id: &str,
        expected_revision: crate::node_system::document::ResourceRevision,
        operation_id: crate::node_system::document::OperationId,
        f: F,
    ) -> Result<crate::event::ResourceMutationCommandResultDto<R>, String>
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
        let data = f(&mut instance, &session)?;
        let mutation = self.commit_database_instance(&session, &token, instance, operation_id)?;
        let result = crate::event::ResourceMutationCommandResultDto { data, mutation };
        reservation.complete();
        Ok(result)
    }

    fn validate_database_project(
        session: &ProjectSession,
        project_instance_id: &crate::project::ProjectInstanceId,
    ) -> Result<(), String> {
        if &session.instance_id != project_instance_id {
            return Err("stale_project_lifecycle: project instance changed".into());
        }
        Ok(())
    }

    pub(crate) fn acquire_database_write_lease(
        &self,
    ) -> Result<(ProjectSession, ProjectFilesystemLeaseSet), String> {
        self.ensure_project_operational()
            .map_err(|error| format!("{}: {error}", error.code()))?;
        let session = self
            .capture_project_session()
            .map_err(|error| error.to_string())?;
        let lease = self
            .filesystem()
            .acquire(session.root.clone())
            .map_err(|error| error.to_string())?;
        self.validate_project_session(&session)
            .map_err(|error| format!("{}: {error}", error.code()))?;
        Ok((session, lease))
    }

    pub(crate) fn database_snapshot_for_session(
        &self,
        session: &ProjectSession,
        id: &str,
    ) -> Result<(DatabaseAuthorityToken, DatabaseInstance), String> {
        self.validate_project_session(session)
            .map_err(|error| format!("{}: {error}", error.code()))?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err("stale_project_lifecycle: project instance changed".into());
        }
        let data = self.project_data.read().unwrap();
        let store = self.project_store.read().unwrap();
        let revisions = self.database_authority_revisions.read().unwrap();
        if !data.databases.contains_key(id) {
            return Err("Database not found".into());
        }
        let instance = store
            .databases
            .get(id)
            .cloned()
            .ok_or_else(|| "Database not found".to_string())?;
        let database_revision = revisions
            .get(id)
            .copied()
            .ok_or_else(|| "stale_project_lifecycle: database authority is missing".to_string())?;
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

    pub(crate) fn revisioned_database_snapshot_for_session(
        &self,
        session: &ProjectSession,
        project_instance_id: &crate::project::ProjectInstanceId,
        id: &str,
        expected_revision: crate::node_system::document::ResourceRevision,
    ) -> Result<(DatabaseAuthorityToken, DatabaseInstance), String> {
        Self::validate_database_project(session, project_instance_id)?;
        let (token, instance) = self.database_snapshot_for_session(session, id)?;
        if token.database_revision != expected_revision.get() {
            return Err("stale_database_revision: database authority conflict".into());
        }
        Ok((token, instance))
    }

    fn validate_database_authority(
        publication: &super::project_state::MutationPublication,
        session: &ProjectSession,
        current_session_id: &crate::node_system::analysis::ProjectSessionId,
        revisions: &std::collections::HashMap<String, u64>,
        token: &DatabaseAuthorityToken,
        id: &str,
    ) -> Result<(), String> {
        if publication.project_instance_id != session.instance_id.as_str()
            || publication.project_instance_id != token.project_instance_id
            || current_session_id != &token.project_session_id
            || token.database_id != id
            || revisions.get(id).copied() != Some(token.database_revision)
        {
            return Err("stale_project_lifecycle: database authority conflict".into());
        }
        Ok(())
    }

    fn publish_database_delta(
        &self,
        publication: &mut super::project_state::MutationPublication,
        revisions: &mut std::collections::HashMap<String, u64>,
        id: &str,
        from_revision: crate::node_system::document::ResourceRevision,
        operation_id: crate::node_system::document::OperationId,
        before: Option<DatabaseDecl>,
        after: Option<DatabaseDecl>,
    ) -> crate::event::ResourceMutationResultDto {
        use crate::node_system::document::{
            DatabaseDocumentPatch, DatabaseResourceKey, ResourceDeltaEvent, ResourceDocumentPatch,
            ResourceKey,
        };

        let publication_revision = publication.allocate_resource_revision();
        let to_revision = from_revision.next();
        revisions.insert(id.to_string(), to_revision.get());
        crate::event::ResourceMutationResultDto {
            operation_id,
            project_instance_id: publication.project_instance_id.clone(),
            publication_revision,
            moves: Vec::new(),
            deltas: vec![ResourceDeltaEvent {
                resource: ResourceKey::Database(DatabaseResourceKey(
                    format!("databases/{id}").into(),
                )),
                from_revision,
                to_revision,
                caused_by: Some(operation_id),
                payload: ResourceDocumentPatch::Database(DatabaseDocumentPatch { before, after }),
            }],
            worksheet_deltas: Vec::new(),
            projection_replacements: Vec::new(),
            projection_status: crate::event::ProjectionStatusDto::Complete {
                expected_graph_paths: Vec::new(),
            },
            history: self.history.read().unwrap().status(),
        }
    }

    pub(crate) fn commit_database_instance(
        &self,
        session: &ProjectSession,
        token: &DatabaseAuthorityToken,
        instance: DatabaseInstance,
        operation_id: crate::node_system::document::OperationId,
    ) -> Result<crate::event::ResourceMutationResultDto, String> {
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
            .ok_or("Database not found")?;
        if !store.databases.contains_key(id) {
            return Err("Database not found".into());
        }
        let after = instance.decl.clone();
        data.databases.insert(id.to_string(), after.clone());
        store.databases.insert(id.to_string(), instance);
        let mutation = self.publish_database_delta(
            &mut publication,
            &mut revisions,
            id,
            crate::node_system::document::ResourceRevision::new(token.database_revision),
            operation_id,
            Some(before),
            Some(after),
        );
        Ok(mutation)
    }

    #[cfg(test)]
    pub(crate) fn add_database_for_session(
        &self,
        session: &ProjectSession,
        project_instance_id: &crate::project::ProjectInstanceId,
        operation_id: crate::node_system::document::OperationId,
        instance: DatabaseInstance,
    ) -> Result<crate::event::ResourceMutationResultDto, String> {
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

    pub(crate) fn commit_database_add_for_session(
        &self,
        session: &ProjectSession,
        project_instance_id: &crate::project::ProjectInstanceId,
        operation_id: crate::node_system::document::OperationId,
        instance: DatabaseInstance,
    ) -> Result<crate::event::ResourceMutationResultDto, String> {
        Self::validate_database_project(session, project_instance_id)?;
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err("stale_project_lifecycle: project instance changed".into());
        }
        let decl = instance.decl.clone();
        let id = decl.id.clone();
        let mut data = self.project_data.write().unwrap();
        let mut store = self.project_store.write().unwrap();
        let mut revisions = self.database_authority_revisions.write().unwrap();
        if data.databases.contains_key(&id) || store.databases.contains_key(&id) {
            return Err("database_already_exists: expected database to be absent".into());
        }
        data.databases.insert(id.clone(), decl.clone());
        store.databases.insert(id.clone(), instance);
        let from_revision = revisions
            .get(&id)
            .copied()
            .map(crate::node_system::document::ResourceRevision::new)
            .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL);
        let mutation = self.publish_database_delta(
            &mut publication,
            &mut revisions,
            &id,
            from_revision,
            operation_id,
            None,
            Some(decl),
        );
        Ok(mutation)
    }

    pub(crate) fn commit_database_name(
        &self,
        session: &ProjectSession,
        token: &DatabaseAuthorityToken,
        id: &str,
        name: &str,
        operation_id: crate::node_system::document::OperationId,
    ) -> Result<crate::event::ResourceMutationResultDto, String> {
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
        let declaration = data.databases.get_mut(id).ok_or("Database not found")?;
        let before = declaration.clone();
        let instance = store.databases.get_mut(id).ok_or("Database not found")?;
        declaration.name = Some(name.to_string());
        instance.decl.name = Some(name.to_string());
        let after = declaration.clone();
        let mutation = self.publish_database_delta(
            &mut publication,
            &mut revisions,
            id,
            crate::node_system::document::ResourceRevision::new(token.database_revision),
            operation_id,
            Some(before),
            Some(after),
        );
        Ok(mutation)
    }

    pub fn delete_database(
        &self,
        project_instance_id: &crate::project::ProjectInstanceId,
        id: &str,
        expected_revision: crate::node_system::document::ResourceRevision,
        operation_id: crate::node_system::document::OperationId,
    ) -> Result<crate::event::ResourceMutationCommandResultDto<()>, String> {
        let reservation = self.reserve_database_operation(project_instance_id, operation_id)?;
        let (session, _lease) = self.acquire_database_write_lease()?;
        let (token, instance) = self.revisioned_database_snapshot_for_session(
            &session,
            project_instance_id,
            id,
            expected_revision,
        )?;
        crate::application::database::remove_duckdb_table_if_needed(
            &instance.decl.engine,
            Some(session.root.as_path()),
        )?;
        let mutation = self.commit_database_delete(&session, &token, id, operation_id)?;
        let result = crate::event::ResourceMutationCommandResultDto { data: (), mutation };
        reservation.complete();
        Ok(result)
    }

    pub(crate) fn commit_database_delete(
        &self,
        session: &ProjectSession,
        token: &DatabaseAuthorityToken,
        id: &str,
        operation_id: crate::node_system::document::OperationId,
    ) -> Result<crate::event::ResourceMutationResultDto, String> {
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
        let before = data.databases.remove(id).ok_or("Database not found")?;
        if store.databases.remove(id).is_none() {
            data.databases.insert(id.to_string(), before);
            return Err("Database not found".into());
        }
        let mutation = self.publish_database_delta(
            &mut publication,
            &mut revisions,
            id,
            crate::node_system::document::ResourceRevision::new(token.database_revision),
            operation_id,
            Some(before),
            None,
        );
        Ok(mutation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

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
        crate::node_system::document::ResourceRevision,
        crate::node_system::document::OperationId,
    ) {
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let revision = state
            .database_authority_revisions
            .read()
            .unwrap()
            .get(id)
            .copied()
            .map(crate::node_system::document::ResourceRevision::new)
            .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL);
        (
            project_instance_id,
            revision,
            crate::node_system::document::OperationId::new(),
        )
    }

    fn add_database_fixture(
        state: &ProjectState,
        instance: DatabaseInstance,
    ) -> Result<crate::event::ResourceMutationResultDto, String> {
        let (session, _lease) = state.acquire_database_write_lease()?;
        state.add_database_for_session(
            &session,
            &session.instance_id,
            crate::node_system::document::OperationId::new(),
            instance,
        )
    }

    fn load_database_fixture(
        state: &ProjectState,
        engine: crate::schema::DatabaseEngineDTO,
    ) -> Result<crate::application::database::LoadDatabaseResult, String> {
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        crate::application::database::load_database(
            state,
            &project_instance_id,
            crate::node_system::document::OperationId::new(),
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
                id: "sales".into(),
                engine: DatabaseEngine::InMemory {
                    name: "sales".into(),
                },
                schema_version: 1,
                required: false,
                name: Some("Original".into()),
            },
        );
        state.activate_project_fixture("database-read-original".into(), original);
        let stale = state.capture_project_session().unwrap().instance_id;
        let mut replacement = crate::project::ProjectData::new();
        replacement.databases.insert(
            "sales".into(),
            DatabaseDecl {
                id: "sales".into(),
                engine: DatabaseEngine::InMemory {
                    name: "sales".into(),
                },
                schema_version: 1,
                required: false,
                name: Some("Replacement".into()),
            },
        );
        state.activate_project_fixture("database-read-replacement".into(), replacement);
        let closure_called = std::sync::atomic::AtomicBool::new(false);

        let result = state.with_database_snapshot_for_project(&stale, "sales", |_| {
            closure_called.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        assert_eq!(result.unwrap_err().code(), "stale_project_lifecycle");
        assert!(!closure_called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn execution_access_during_activation_is_detached_and_failure_has_zero_effects() {
        let root = project_root("detached-execution");
        let csv = root.join("execution.csv");
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
        let prepared = state.prepare_project_activation(Some(&root)).unwrap();
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let entered_for_hook = Arc::clone(&entered);
        let release_for_hook = Arc::clone(&release);
        state.set_activation_final_rebuild_test_hook(Arc::new(move || {
            entered_for_hook.wait();
            release_for_hook.wait();
        }));
        let activation_state = state.clone();
        let activation =
            std::thread::spawn(move || activation_state.activate_prepared_project(prepared));
        entered.wait();

        let generation_before_access = state.authority_generation_for_test();
        let execution = state.access_database(&imported.id, DatabaseAccess::Execution);
        let shared_remained_duckdb = matches!(
            state
                .project_store
                .read()
                .unwrap()
                .databases
                .get(&imported.id)
                .unwrap()
                .state,
            DatabaseState::DuckDb { .. }
        );
        let generation_after_access = state.authority_generation_for_test();
        release.wait();
        activation.join().unwrap().unwrap();

        assert!(execution.is_ok());
        assert!(shared_remained_duckdb);
        assert_eq!(generation_after_access, generation_before_access);

        let missing_id = "missing-execution".to_string();
        add_database_fixture(
            &state,
            DatabaseInstance {
                decl: DatabaseDecl {
                    id: missing_id.clone(),
                    engine: DatabaseEngine::InMemory {
                        name: "missing".into(),
                    },
                    schema_version: 1,
                    required: false,
                    name: Some("missing".into()),
                },
                state: DatabaseState::DuckDb {
                    duckdb_path: root.join("missing.duckdb").to_string_lossy().into_owned(),
                    table: "missing".into(),
                    row_count: 1,
                    columns: Vec::new(),
                    history: EditHistory::new(),
                },
            },
        )
        .unwrap();
        let generation_before_failure = state.authority_generation_for_test();

        assert!(
            state
                .access_database(&missing_id, DatabaseAccess::Execution)
                .is_err()
        );
        assert_eq!(
            state.authority_generation_for_test(),
            generation_before_failure
        );
        assert!(matches!(
            state
                .project_store
                .read()
                .unwrap()
                .databases
                .get(&missing_id)
                .unwrap()
                .state,
            DatabaseState::DuckDb { .. }
        ));

        let _ = std::fs::remove_dir_all(root);
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
                    id: database_id.into(),
                    engine: DatabaseEngine::InMemory {
                        name: "writer".into(),
                    },
                    schema_version: 1,
                    required: false,
                    name: Some("Before".into()),
                },
                state: DatabaseState::Failed {
                    error: "fixture".into(),
                },
            },
        )
        .unwrap();
        let graph_path =
            crate::project::GraphResourcePath::new("events/DatabaseWriter.yssbi-event").unwrap();
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
        let document_path =
            crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        assert!(coordinator.contains_slot_for_test(&document_path));
        let generation = state.authority_generation_for_test();

        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, database_id);
        let closure_had_no_store_lock = state
            .with_database_mut(
                &project_instance_id,
                database_id,
                expected_revision,
                operation_id,
                |database| {
                    let store_lock = state.project_store.try_write();
                    let lock_available = store_lock.is_ok();
                    drop(store_lock);
                    database.decl.name = Some("Committed".into());
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
                .as_deref(),
            Some("Committed")
        );
        assert_eq!(
            state.get_data().unwrap().databases[database_id]
                .name
                .as_deref(),
            Some("Committed")
        );
        assert_eq!(state.authority_generation_for_test(), generation + 1);
        assert!(coordinator.contains_slot_for_test(&document_path));

        state.graph_projection(&graph_path, "en-US").unwrap();
        let generation = state.authority_generation_for_test();
        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, database_id);
        let error = state
            .with_database_mut(
                &project_instance_id,
                database_id,
                expected_revision,
                operation_id,
                |database| {
                    database.decl.name = Some("Rejected".into());
                    Err::<(), _>("reject mutation".into())
                },
            )
            .unwrap_err();
        assert_eq!(error, "reject mutation");
        assert_eq!(
            state.project_store.read().unwrap().databases[database_id]
                .decl
                .name
                .as_deref(),
            Some("Committed")
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
                    id: database_id.into(),
                    engine: DatabaseEngine::InMemory {
                        name: "snapshot".into(),
                    },
                    schema_version: 1,
                    required: false,
                    name: Some("Authoritative".into()),
                },
                state: DatabaseState::Failed {
                    error: "fixture".into(),
                },
            },
        )
        .unwrap();
        let graph_path =
            crate::project::GraphResourcePath::new("events/DatabaseSnapshot.yssbi-event").unwrap();
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
        let document_path =
            crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        assert!(coordinator.contains_slot_for_test(&document_path));
        let generation = state.authority_generation_for_test();

        let closure_had_no_store_lock = state
            .with_database_snapshot(database_id, |database| {
                let store_lock = state.project_store.try_write();
                let lock_available = store_lock.is_ok();
                drop(store_lock);
                database.decl.name = Some("Local only".into());
                Ok(lock_available)
            })
            .unwrap();
        state
            .with_database_snapshot(database_id, |database| {
                assert_eq!(database.decl.name.as_deref(), Some("Authoritative"));
                Ok(())
            })
            .unwrap();

        assert!(closure_had_no_store_lock);
        assert_eq!(
            state.project_store.read().unwrap().databases[database_id]
                .decl
                .name
                .as_deref(),
            Some("Authoritative")
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
            crate::node_system::document::OperationId::new(),
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
        let metadata =
            crate::application::database::get_database_meta(&state, &imported.id).unwrap();
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
                    id: id.into(),
                    engine: DatabaseEngine::InMemory { name: id.into() },
                    schema_version: 1,
                    required: false,
                    name: Some(name.into()),
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
                crate::project::GraphResourcePath::new("events/Unrelated.yssbi-event").unwrap(),
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
                crate::node_system::document::OperationId::new(),
            )
            .unwrap();

        assert_eq!(
            state.project_store.read().unwrap().databases["writer"]
                .decl
                .name
                .as_deref(),
            Some("Committed")
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
                crate::project::GraphResourcePath::new("events/UnrelatedRename.yssbi-event")
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
                .as_deref(),
            Some("Renamed")
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
            instance.decl.name = Some("Stale A".into());
            stale_state.commit_database_instance(
                &stale_session,
                &token,
                instance,
                crate::node_system::document::OperationId::new(),
            )
        });
        snapshot_ready_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, "writer");
        state
            .with_database_mut(
                &project_instance_id,
                "writer",
                expected_revision,
                operation_id,
                |database| {
                    database.decl.name = Some("Committed B".into());
                    Ok(())
                },
            )
            .unwrap();
        let generation_after_b = state.authority_generation_for_test();
        resume_tx.send(()).unwrap();

        let error = stale.join().unwrap().unwrap_err();
        assert!(error.contains("conflict") || error.contains("stale"));
        assert_eq!(state.authority_generation_for_test(), generation_after_b);
        assert_eq!(
            state.project_store.read().unwrap().databases["writer"]
                .decl
                .name
                .as_deref(),
            Some("Committed B")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn database_operation_ids_reject_in_flight_and_completed_replays_and_release_failures() {
        use crate::node_system::document::{OperationId, ResourceRevision};
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
                    database.decl.name = Some("First".into());
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
        assert!(in_flight.unwrap_err().contains("duplicate_operation"));
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
        assert!(replay.unwrap_err().contains("duplicate_operation"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let retryable_operation = OperationId::new();
        let failed = state.with_database_writer(
            &project_instance_id,
            "writer",
            committed_revision,
            retryable_operation,
            |_, _| Err::<(), _>("injected failure".into()),
        );
        assert_eq!(failed.unwrap_err(), "injected failure");
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
        use crate::node_system::document::{
            DatabaseResourceKey, OperationId, ResourceDocumentPatch, ResourceKey, ResourceRevision,
        };

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
                        id: database_id.into(),
                        engine: DatabaseEngine::InMemory {
                            name: database_id.into(),
                        },
                        schema_version: 1,
                        required: false,
                        name: Some("Before".into()),
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
        assert!(
            duplicate
                .unwrap_err()
                .contains("expected database to be absent")
        );
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
                database.decl.name = Some("Rejected".into());
                Err::<(), _>("reject mutation".into())
            },
        );
        assert_eq!(failure.unwrap_err(), "reject mutation");
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
        assert!(stale.unwrap_err().contains("stale"));
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
                id: "replacement".into(),
                engine: DatabaseEngine::InMemory {
                    name: "replacement".into(),
                },
                schema_version: 1,
                required: false,
                name: Some("Replacement".into()),
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
            .with_database_mut(
                &project_instance_id,
                "writer",
                expected_revision,
                operation_id,
                |database| {
                    database.decl.name = Some("New Authority".into());
                    Ok(())
                },
            )
            .unwrap();
        let generation = state.authority_generation_for_test();
        assert!(
            state
                .commit_database_name(
                    &session,
                    &rename_token,
                    "writer",
                    "Stale Rename",
                    crate::node_system::document::OperationId::new(),
                )
                .unwrap_err()
                .contains("stale")
        );
        assert_eq!(state.authority_generation_for_test(), generation);

        let (delete_token, _) = state
            .database_snapshot_for_session(&session, "writer")
            .unwrap();
        let (project_instance_id, expected_revision, operation_id) =
            database_basis(&state, "writer");
        state
            .with_database_mut(
                &project_instance_id,
                "writer",
                expected_revision,
                operation_id,
                |database| {
                    database.decl.required = true;
                    Ok(())
                },
            )
            .unwrap();
        let generation = state.authority_generation_for_test();
        assert!(
            state
                .commit_database_delete(
                    &session,
                    &delete_token,
                    "writer",
                    crate::node_system::document::OperationId::new(),
                )
                .unwrap_err()
                .contains("stale")
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
