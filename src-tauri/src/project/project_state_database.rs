use super::ProjectState;
use crate::database::*;
use crate::project::{ProjectFilesystemLeaseSet, ProjectSession};
use polars::prelude::*;

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

    pub fn with_database_mut<F, R>(&self, id: &str, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut DatabaseInstance) -> Result<R, String>,
    {
        self.ensure_project_operational()
            .map_err(|error| format!("{}: {error}", error.code()))?;
        let mut store = self.project_store.write().unwrap();
        let db = store
            .databases
            .get_mut(id)
            .ok_or_else(|| "Database not found".to_string())?;
        f(db)
    }

    pub fn with_database_writer<F, R>(&self, id: &str, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut DatabaseInstance, &ProjectSession) -> Result<R, String>,
    {
        let (session, _lease) = self.acquire_database_write_lease()?;
        let mut instance = self.database_snapshot_for_session(&session, id)?;
        let result = f(&mut instance, &session)?;
        self.commit_database_instance(&session, id, instance)?;
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
    ) -> Result<DatabaseInstance, String> {
        self.validate_project_session(session)
            .map_err(|error| format!("{}: {error}", error.code()))?;
        self.project_store
            .read()
            .unwrap()
            .databases
            .get(id)
            .cloned()
            .ok_or_else(|| "Database not found".to_string())
    }

    pub(crate) fn commit_database_instance(
        &self,
        session: &ProjectSession,
        id: &str,
        instance: DatabaseInstance,
    ) -> Result<(), String> {
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err("stale_project_lifecycle: project instance changed".into());
        }
        let mut data = self.project_data.write().unwrap();
        let mut store = self.project_store.write().unwrap();
        if !data.databases.contains_key(id) || !store.databases.contains_key(id) {
            return Err("Database not found".into());
        }
        data.databases.insert(id.to_string(), instance.decl.clone());
        store.databases.insert(id.to_string(), instance);
        publication.advance_authority_generation();
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
        data.databases.insert(id.clone(), decl);
        store.databases.insert(id, instance);
        publication.advance_authority_generation();
        self.invalidate_graph_runtime();
        Ok(())
    }

    pub fn add_database(&self, instance: DatabaseInstance) -> Result<(), String> {
        let (session, _lease) = self.acquire_database_write_lease()?;
        self.add_database_for_session(&session, instance)
    }

    pub(crate) fn commit_database_name(
        &self,
        session: &ProjectSession,
        id: &str,
        name: &str,
    ) -> Result<(), String> {
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err("stale_project_lifecycle: project instance changed".into());
        }
        let mut data = self.project_data.write().unwrap();
        let mut store = self.project_store.write().unwrap();
        let declaration = data.databases.get_mut(id).ok_or("Database not found")?;
        let instance = store.databases.get_mut(id).ok_or("Database not found")?;
        declaration.name = Some(name.to_string());
        instance.decl.name = Some(name.to_string());
        publication.advance_authority_generation();
        self.invalidate_graph_runtime();
        Ok(())
    }

    pub fn delete_database(&self, id: &str) -> Result<(), String> {
        let (session, _lease) = self.acquire_database_write_lease()?;
        let engine = self
            .project_data
            .read()
            .unwrap()
            .databases
            .get(id)
            .map(|decl| decl.engine.clone())
            .ok_or("Database not found")?;
        crate::application::database::remove_duckdb_table_if_needed(
            &engine,
            Some(session.root.as_path()),
        )?;

        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err("stale_project_lifecycle: project instance changed".into());
        }
        let mut data = self.project_data.write().unwrap();
        let mut store = self.project_store.write().unwrap();
        data.databases.remove(id);
        store.databases.remove(id);
        publication.advance_authority_generation();
        self.invalidate_graph_runtime();
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
}
