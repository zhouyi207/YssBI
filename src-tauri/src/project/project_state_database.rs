use super::ProjectState;
use crate::database::*;
use crate::project::{ProjectFilesystemLeaseSet, ProjectSession};
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

    /// Runs and commits an authoritative database mutation.
    ///
    /// The closure operates on a detached instance without project locks. A
    /// successful result is published through the central database commit path.
    pub fn with_database_mut<F, R>(&self, id: &str, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut DatabaseInstance) -> Result<R, String>,
    {
        self.with_database_writer(id, |database, _| f(database))
    }

    /// Runs a read/query operation against a detached database snapshot.
    ///
    /// Mutations to the snapshot are discarded. This API is crate-visible so
    /// external mutation callers use `with_database_mut` and its commit path.
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

    pub fn with_database_writer<F, R>(&self, id: &str, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut DatabaseInstance, &ProjectSession) -> Result<R, String>,
    {
        let (session, _lease) = self.acquire_database_write_lease()?;
        let (token, mut instance) = self.database_snapshot_for_session(&session, id)?;
        let result = f(&mut instance, &session)?;
        self.commit_database_instance(&session, &token, instance)?;
        Ok(result)
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

    fn advance_database_authority(
        publication: &mut super::project_state::MutationPublication,
        revisions: &mut std::collections::HashMap<String, u64>,
        id: &str,
    ) {
        publication.advance_authority_generation();
        revisions.insert(id.to_string(), publication.authority_generation());
    }

    pub(crate) fn commit_database_instance(
        &self,
        session: &ProjectSession,
        token: &DatabaseAuthorityToken,
        instance: DatabaseInstance,
    ) -> Result<(), String> {
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
        if !data.databases.contains_key(id) || !store.databases.contains_key(id) {
            return Err("Database not found".into());
        }
        data.databases.insert(id.to_string(), instance.decl.clone());
        store.databases.insert(id.to_string(), instance);
        Self::advance_database_authority(&mut publication, &mut revisions, id);
        self.invalidate_all_compile_products();
        Ok(())
    }

    pub(crate) fn add_database_for_session(
        &self,
        session: &ProjectSession,
        instance: DatabaseInstance,
    ) -> Result<(), String> {
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err("stale_project_lifecycle: project instance changed".into());
        }
        let decl = instance.decl.clone();
        let id = decl.id.clone();
        let mut data = self.project_data.write().unwrap();
        let mut store = self.project_store.write().unwrap();
        let mut revisions = self.database_authority_revisions.write().unwrap();
        data.databases.insert(id.clone(), decl);
        store.databases.insert(id.clone(), instance);
        Self::advance_database_authority(&mut publication, &mut revisions, &id);
        self.invalidate_all_compile_products();
        Ok(())
    }

    pub fn add_database(&self, instance: DatabaseInstance) -> Result<(), String> {
        let (session, _lease) = self.acquire_database_write_lease()?;
        self.add_database_for_session(&session, instance)
    }

    pub(crate) fn commit_database_name(
        &self,
        session: &ProjectSession,
        token: &DatabaseAuthorityToken,
        id: &str,
        name: &str,
    ) -> Result<(), String> {
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
        let instance = store.databases.get_mut(id).ok_or("Database not found")?;
        declaration.name = Some(name.to_string());
        instance.decl.name = Some(name.to_string());
        Self::advance_database_authority(&mut publication, &mut revisions, id);
        self.invalidate_all_compile_products();
        Ok(())
    }

    pub fn delete_database(&self, id: &str) -> Result<(), String> {
        let (session, _lease) = self.acquire_database_write_lease()?;
        let (token, instance) = self.database_snapshot_for_session(&session, id)?;
        crate::application::database::remove_duckdb_table_if_needed(
            &instance.decl.engine,
            Some(session.root.as_path()),
        )?;
        self.commit_database_delete(&session, &token, id)
    }

    pub(crate) fn commit_database_delete(
        &self,
        session: &ProjectSession,
        token: &DatabaseAuthorityToken,
        id: &str,
    ) -> Result<(), String> {
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
        if !data.databases.contains_key(id) || !store.databases.contains_key(id) {
            return Err("Database not found".into());
        }
        data.databases.remove(id);
        store.databases.remove(id);
        publication.advance_authority_generation();
        revisions.remove(id);
        self.invalidate_all_compile_products();
        Ok(())
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

    #[test]
    fn execution_access_during_activation_is_detached_and_failure_has_zero_effects() {
        let root = project_root("detached-execution");
        let csv = root.join("execution.csv");
        std::fs::write(&csv, "value\n1\n").unwrap();
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        let imported = crate::application::database::load_database(
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
        state
            .add_database(DatabaseInstance {
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
            })
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
        state
            .add_database(DatabaseInstance {
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
            })
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

        let closure_had_no_store_lock = state
            .with_database_mut(database_id, |database| {
                let store_lock = state.project_store.try_write();
                let lock_available = store_lock.is_ok();
                drop(store_lock);
                database.decl.name = Some("Committed".into());
                Ok(lock_available)
            })
            .unwrap();

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
        assert!(!coordinator.contains_slot_for_test(&document_path));

        state.graph_projection(&graph_path, "en-US").unwrap();
        let generation = state.authority_generation_for_test();
        let error = state
            .with_database_mut(database_id, |database| {
                database.decl.name = Some("Rejected".into());
                Err::<(), _>("reject mutation".into())
            })
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
        state
            .add_database(DatabaseInstance {
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
            })
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

        let imported = crate::application::database::load_database(
            &state,
            crate::schema::DatabaseEngineDTO::Csv {
                path: csv.to_string_lossy().into_owned(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: None,
            },
        )
        .unwrap();
        let after_import = state.authority_generation_for_test();
        assert!(after_import > initial);

        crate::application::database::rename_database(&state, &imported.id, "renamed").unwrap();
        let after_rename = state.authority_generation_for_test();
        assert!(after_rename > after_import);

        state
            .with_database_writer(&imported.id, |database, _| {
                database.add_column("added", "Int64")
            })
            .unwrap();
        let after_edit = state.authority_generation_for_test();
        assert!(after_edit > after_rename);
        let metadata =
            crate::application::database::get_database_meta(&state, &imported.id).unwrap();
        assert!(metadata.columns.iter().any(|column| column.name == "added"));

        crate::application::database::save_database_changes(&state, &imported.id).unwrap();
        let after_save = state.authority_generation_for_test();
        assert!(after_save > after_edit);

        state.delete_database(&imported.id).unwrap();
        assert!(state.authority_generation_for_test() > after_save);
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
        state
            .add_database(DatabaseInstance {
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
            })
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
            .commit_database_name(&session, &token, "writer", "Committed")
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
        let imported = crate::application::database::load_database(
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
        let rename = std::thread::spawn(move || {
            crate::application::database::rename_database(&rename_state, &rename_id, "Renamed")
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
            stale_state.commit_database_instance(&stale_session, &token, instance)
        });
        snapshot_ready_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        state
            .with_database_mut("writer", |database| {
                database.decl.name = Some("Committed B".into());
                Ok(())
            })
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
    fn database_rename_and_delete_share_authority_token_cas() {
        let root = project_root("rename-delete-cas");
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        add_in_memory_database(&state, "writer", "Initial");
        let session = state.capture_project_session().unwrap();

        let (rename_token, _) = state
            .database_snapshot_for_session(&session, "writer")
            .unwrap();
        state
            .with_database_mut("writer", |database| {
                database.decl.name = Some("New Authority".into());
                Ok(())
            })
            .unwrap();
        let generation = state.authority_generation_for_test();
        assert!(
            state
                .commit_database_name(&session, &rename_token, "writer", "Stale Rename")
                .unwrap_err()
                .contains("stale")
        );
        assert_eq!(state.authority_generation_for_test(), generation);

        let (delete_token, _) = state
            .database_snapshot_for_session(&session, "writer")
            .unwrap();
        state
            .with_database_mut("writer", |database| {
                database.decl.required = true;
                Ok(())
            })
            .unwrap();
        let generation = state.authority_generation_for_test();
        assert!(
            state
                .commit_database_delete(&session, &delete_token, "writer")
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
