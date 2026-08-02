use crate::application::database::bind_duckdb_instance;
use crate::database::{DatabaseEngine, DatabaseInstance, DatabaseState};
use crate::node_system::document::ResourceRevision;
use crate::project::{
    GraphResourcePath, NormalizedProjectRoot, ProjectData, ProjectFilesystemError,
    ProjectInstanceId, ProjectSession, ProjectState, ProjectStore,
};
use crate::tabular::{normalize_variable_tabular, sync_variable_cache};
use crate::variable::VariableId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone, Default)]
pub(crate) struct ProjectActivationCoordinator {
    shared: Arc<ProjectActivationAdmission>,
}

#[derive(Default)]
struct ProjectActivationAdmission {
    owned: Mutex<bool>,
    available: Condvar,
}

pub(crate) struct ProjectActivationToken {
    shared: Arc<ProjectActivationAdmission>,
}

impl ProjectActivationCoordinator {
    pub(crate) fn acquire(&self) -> ProjectActivationToken {
        let mut owned = self
            .shared
            .owned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *owned {
            owned = self
                .shared
                .available
                .wait(owned)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *owned = true;
        ProjectActivationToken {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Drop for ProjectActivationToken {
    fn drop(&mut self) {
        let mut owned = self
            .shared
            .owned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *owned = false;
        drop(owned);
        self.shared.available.notify_one();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedAuthorityBasis {
    pub project_instance_id: ProjectInstanceId,
    pub project_root: NormalizedProjectRoot,
    pub publication_revision: u64,
    pub authority_generation: u64,
}

pub struct PreparedProjectActivation {
    pub session_root: Option<NormalizedProjectRoot>,
    pub data: ProjectData,
    pub store: ProjectStore,
    pub(crate) variable_revisions:
        HashMap<VariableId, crate::project::project_state::VariableRevisionEntry>,
    pub(crate) graph_revisions: HashMap<GraphResourcePath, ResourceRevision>,
    pub(crate) worksheet_revisions: HashMap<String, ResourceRevision>,
    pub(crate) authority_basis: Option<PreparedAuthorityBasis>,
    pub(crate) requires_final_rebuild: bool,
}

impl PreparedProjectActivation {
    pub(super) fn from_data(
        session_root: Option<NormalizedProjectRoot>,
        mut data: ProjectData,
        authority_basis: Option<PreparedAuthorityBasis>,
        requires_final_rebuild: bool,
    ) -> Self {
        let mut store = ProjectStore::default();
        for (id, declaration) in &data.databases {
            let instance = if matches!(declaration.engine, DatabaseEngine::DuckDb { .. }) {
                bind_duckdb_instance(
                    declaration,
                    session_root.as_ref().map(NormalizedProjectRoot::as_path),
                )
            } else {
                DatabaseInstance {
                    decl: declaration.clone(),
                    state: DatabaseState::Failed {
                        error: "Only DuckDb datasets are supported; re-import the data".into(),
                    },
                }
            };
            store.databases.insert(id.clone(), instance);
        }
        for variable in data.variables.values_mut() {
            if normalize_variable_tabular(variable).is_ok() {
                let _ = sync_variable_cache(&mut store, variable);
            }
        }
        let graph_revisions = data
            .graphs
            .iter()
            .map(|(path, resource)| (path.clone(), resource.document.revision))
            .collect();
        let variable_revisions = data
            .variables
            .keys()
            .copied()
            .map(|id| {
                (
                    id,
                    crate::project::project_state::VariableRevisionEntry::present(
                        ResourceRevision::INITIAL,
                    ),
                )
            })
            .collect();
        let worksheet_revisions = data
            .worksheets
            .iter()
            .map(|(id, document)| (id.clone(), document.revision))
            .collect();
        Self {
            session_root,
            data,
            store,
            variable_revisions,
            graph_revisions,
            worksheet_revisions,
            authority_basis,
            requires_final_rebuild,
        }
    }
}

impl ProjectState {
    pub fn prepare_project_activation(
        &self,
        path: Option<&Path>,
    ) -> Result<PreparedProjectActivation, ProjectFilesystemError> {
        let Some(path) = path else {
            return Ok(PreparedProjectActivation::from_data(
                None,
                ProjectData::new(),
                None,
                false,
            ));
        };
        let root = NormalizedProjectRoot::from_project_path(path)?;
        let lease = self.filesystem().acquire(root.clone())?;
        let authority_before = self.capture_prepared_authority_basis(&root)?;
        let data = self.read_activation_data(&root)?;
        self.run_activation_preparation_after_read_test_hook();
        let authority_after = self.capture_prepared_authority_basis(&root)?;
        if authority_before != authority_after {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project authority changed during activation preparation".into(),
            });
        }
        let prepared =
            PreparedProjectActivation::from_data(Some(root), data, authority_after, true);
        drop(lease);
        Ok(prepared)
    }

    pub fn activate_prepared_project(
        &self,
        mut prepared: PreparedProjectActivation,
    ) -> Result<ProjectSession, ProjectFilesystemError> {
        let root =
            prepared
                .session_root
                .clone()
                .ok_or_else(|| ProjectFilesystemError::InvalidRoot {
                    path: PathBuf::new(),
                    message: "a pathless activation must use clear_project".into(),
                })?;
        let _activation = self.project_activation.acquire();
        let (runs, project_session_id) = self.current_run_registry();
        let _drain_guard = runs.begin_drain(&project_session_id);
        self.run_project_activation_test_hook();
        let lease = self.filesystem().acquire(root.clone())?;
        if prepared.requires_final_rebuild {
            let authority_basis = prepared.authority_basis.take();
            let data = self.read_activation_data(&root)?;
            prepared = PreparedProjectActivation::from_data(
                Some(root.clone()),
                data,
                authority_basis,
                true,
            );
        }
        self.run_activation_final_rebuild_test_hook();
        let published = self.publish_project_activation(prepared)?;
        drop(lease);
        let instance_id = published.dispose();
        Ok(ProjectSession { instance_id, root })
    }

    pub fn activate_project_from_path(
        &self,
        path: &Path,
    ) -> Result<ProjectSession, ProjectFilesystemError> {
        let prepared = self.prepare_project_activation(Some(path))?;
        self.activate_prepared_project(prepared)
    }

    pub fn clear_project(&self) -> Result<ProjectInstanceId, ProjectFilesystemError> {
        let prepared = self.prepare_project_activation(None)?;
        let _activation = self.project_activation.acquire();
        let (runs, project_session_id) = self.current_run_registry();
        let _drain_guard = runs.begin_drain(&project_session_id);
        self.run_project_activation_test_hook();
        let published = self.publish_project_activation(prepared)?;
        Ok(published.dispose())
    }

    #[cfg(test)]
    pub(crate) fn activate_project_fixture(&self, path: String, data: ProjectData) {
        let root = NormalizedProjectRoot::from_project_path(path).unwrap();
        self.activate_prepared_project(PreparedProjectActivation::from_data(
            Some(root),
            data,
            None,
            false,
        ))
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::value::{DataType, DataValue};
    use crate::node_system::runtime::NOOP_RUN_EVENT_SINK;
    use crate::project::{
        GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData, ProjectState,
        WorksheetDocument, fixtures, load_project_from_file,
    };
    use crate::variable::VariableScope;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Barrier};
    use std::time::Duration;

    fn project_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "yssbi-project-activation-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn save_named_project(label: &str) -> (std::path::PathBuf, ProjectData) {
        let root = project_root(label);
        let mut data = ProjectData::new();
        data.metadata.project_name = label.to_string();
        let worksheet = WorksheetDocument::new(format!("{label} worksheet"), "database");
        data.worksheets.insert(worksheet.id.clone(), worksheet);
        fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        (root, data)
    }

    #[test]
    fn activation_replaces_old_session_revision_tombstones() {
        let (old_root, _) = save_named_project("revision-tombstones-old");
        let new_root = project_root("revision-tombstones-new");
        let state = ProjectState::new();
        state.activate_project_from_path(&old_root).unwrap();
        let old_graph = state
            .create_graph_resource_fixture("Old Session Graph", GraphDocumentKind::Event)
            .unwrap();
        state.unload_graph_resource(&old_graph).unwrap();

        let mut new_data = ProjectData::new();
        let new_graph = GraphResourcePath::new("events/NewSession.yssbi-event").unwrap();
        new_data.graphs.insert(
            new_graph,
            GraphResourceDocument::new("New Session", GraphDocumentKind::Event),
        );
        let variable = crate::variable::VariableInstance {
            id: crate::variable::VariableId::new(),
            name: "new_session_variable".into(),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(1),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Global,
            tags: Vec::new(),
        };
        new_data.variables.insert(variable.id, variable);
        let worksheet = WorksheetDocument::new("New Session Worksheet", "database");
        new_data.worksheets.insert(worksheet.id.clone(), worksheet);
        let new_root = crate::project::NormalizedProjectRoot::from_project_path(&new_root).unwrap();
        let prepared = super::PreparedProjectActivation::from_data(
            Some(new_root.clone()),
            new_data.clone(),
            None,
            false,
        );
        let prepared_revisions = (
            prepared.graph_revisions.clone(),
            prepared
                .variable_revisions
                .iter()
                .map(|(id, entry)| (*id, entry.revision))
                .collect(),
            prepared.worksheet_revisions.clone(),
        );

        state.activate_prepared_project(prepared).unwrap();

        assert_eq!(state.revision_state_for_test(), prepared_revisions);
        let (graphs, variables, worksheets) = state.revision_state_for_test();
        assert!(!graphs.contains_key(&old_graph));
        assert!(
            variables
                .keys()
                .all(|id| new_data.variables.contains_key(id))
        );
        assert!(
            worksheets
                .keys()
                .all(|id| new_data.worksheets.contains_key(id))
        );

        let _ = std::fs::remove_dir_all(old_root);
        let _ = std::fs::remove_dir_all(new_root.as_path());
    }

    #[test]
    fn activation_and_pre_run_function_loading_complete_without_deadlock() {
        let (old_root, _) = save_named_project("deadlock-old");
        let (new_root, _) = save_named_project("deadlock-new");

        let state = ProjectState::new();
        state.activate_project_from_path(&old_root).unwrap();
        let event = state
            .create_graph_resource_fixture("Loading Caller", GraphDocumentKind::Event)
            .unwrap();
        let old_function = state
            .create_graph_resource_fixture("Loading Callee", GraphDocumentKind::Function)
            .unwrap();
        let session = state.capture_project_session().unwrap();
        state
            .load_graph_projection(&session.instance_id, &event, 1, "en-US")
            .unwrap();

        let (loading_tx, loading_rx) = std::sync::mpsc::channel();
        state.set_function_load_checkpoint(Arc::new(move |cancellation| {
            loading_tx.send(()).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while !cancellation.is_cancelled() && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            assert!(cancellation.is_cancelled());
        }));

        let execution_state = state.clone();
        let (execution_tx, execution_rx) = std::sync::mpsc::channel();
        let execution = std::thread::spawn(move || {
            let result = execution_state.execute_graph(&event, &NOOP_RUN_EVENT_SINK);
            execution_tx.send(()).unwrap();
            result
        });
        loading_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        let activation_state = state.clone();
        let new_root_for_activation = new_root.clone();
        let (activation_tx, activation_rx) = std::sync::mpsc::channel();
        let activation = std::thread::spawn(move || {
            let result = activation_state.activate_project_from_path(&new_root_for_activation);
            activation_tx.send(()).unwrap();
            result
        });

        execution_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        activation_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let execution_error = execution.join().unwrap().unwrap_err();
        assert!(execution_error.contains("cancel"));
        activation.join().unwrap().unwrap();
        let activated = state.get_data().unwrap();
        assert!(!activated.graphs.contains_key(&old_function));
        assert_eq!(activated.worksheets.len(), 1);

        let _ = std::fs::remove_dir_all(old_root);
        let _ = std::fs::remove_dir_all(new_root);
    }

    #[test]
    fn activation_waits_for_old_pre_runs_without_state_or_filesystem_locks() {
        let (old_root, _) = save_named_project("wait-old");
        let (new_root, _) = save_named_project("wait-new");
        let state = ProjectState::new();
        state.activate_project_from_path(&old_root).unwrap();
        let (runs, session_id) = {
            let store = state.project_store.read().unwrap();
            (Arc::clone(&store.runs), store.project_session_id.clone())
        };
        let pre_run = runs
            .track_pre_run(
                session_id,
                crate::node_system::runtime::CancellationToken::new(),
            )
            .unwrap();

        let activation_state = state.clone();
        let new_root_for_activation = new_root.clone();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let activation = std::thread::spawn(move || {
            let result = activation_state.activate_project_from_path(&new_root_for_activation);
            done_tx.send(()).unwrap();
            result
        });

        assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_err());
        assert_eq!(state.get_data().unwrap().metadata.project_name, "wait-old");
        let old_session = state.capture_project_session().unwrap();
        assert_eq!(
            state.get_path().as_deref(),
            Some(old_session.root.as_path().to_string_lossy().as_ref())
        );
        let old_lease = state.filesystem().acquire(old_session.root).unwrap();
        let new_root = crate::project::NormalizedProjectRoot::from_project_path(&new_root).unwrap();
        let new_lease = state.filesystem().acquire(new_root.clone()).unwrap();
        drop(new_lease);
        drop(old_lease);

        drop(pre_run);
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        activation.join().unwrap().unwrap();

        let _ = std::fs::remove_dir_all(old_root);
        let _ = std::fs::remove_dir_all(new_root.as_path());
    }

    #[test]
    fn concurrent_activations_publish_only_complete_sessions() {
        let (initial_root, _) = save_named_project("concurrent-initial");
        let (root_a, _) = save_named_project("concurrent-a");
        let (root_b, _) = save_named_project("concurrent-b");
        let state = ProjectState::new();
        state.activate_project_from_path(&initial_root).unwrap();
        let prepared_a = state.prepare_project_activation(Some(&root_a)).unwrap();
        let prepared_b = state.prepare_project_activation(Some(&root_b)).unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let stopped = Arc::new(AtomicBool::new(false));

        let observer_state = state.clone();
        let observer_stopped = Arc::clone(&stopped);
        let observer = std::thread::spawn(move || {
            while !observer_stopped.load(Ordering::Acquire) {
                let Ok(session) = observer_state.capture_project_session() else {
                    continue;
                };
                let Ok((_, _, _, data)) = observer_state.coherent_project_read_snapshot(&session)
                else {
                    continue;
                };
                let root = session.root.as_path().to_string_lossy();
                if root.contains("concurrent-initial") {
                    assert_eq!(data.metadata.project_name, "concurrent-initial");
                } else if root.contains("concurrent-a") {
                    assert_eq!(data.metadata.project_name, "concurrent-a");
                } else if root.contains("concurrent-b") {
                    assert_eq!(data.metadata.project_name, "concurrent-b");
                } else {
                    panic!("unexpected activation root: {root}");
                }
            }
        });

        let state_a = state.clone();
        let barrier_a = Arc::clone(&barrier);
        let activation_a = std::thread::spawn(move || {
            barrier_a.wait();
            state_a.activate_prepared_project(prepared_a)
        });
        let state_b = state.clone();
        let barrier_b = Arc::clone(&barrier);
        let activation_b = std::thread::spawn(move || {
            barrier_b.wait();
            state_b.activate_prepared_project(prepared_b)
        });
        barrier.wait();
        activation_a.join().unwrap().unwrap();
        activation_b.join().unwrap().unwrap();
        stopped.store(true, Ordering::Release);
        observer.join().unwrap();

        let session = state.capture_project_session().unwrap();
        let (_, _, _, data) = state.coherent_project_read_snapshot(&session).unwrap();
        let (graph_revisions, variable_revisions, worksheet_revisions) =
            state.revision_state_for_test();
        assert!(graph_revisions.is_empty());
        assert!(variable_revisions.is_empty());
        assert_eq!(worksheet_revisions.len(), data.worksheets.len());
        for (id, document) in &data.worksheets {
            assert_eq!(worksheet_revisions.get(id), Some(&document.revision));
        }
        let (runtime_session, identity_session) = state.runtime_identity_sessions_for_test();
        assert_eq!(runtime_session, identity_session);
        assert_eq!(state.graph_lifecycle_entry_count(), 0);
        assert!(state.project_recovery_marker().error().is_none());
        assert_eq!(state.history_status(), Default::default());
        assert_eq!(state.activation_generation_for_test() % 2, 0);
        assert!(state.try_current_pre_run_admission_for_test().is_some());
        assert!(state.try_current_run_admission_for_test().is_some());
        let root = session.root.as_path().to_string_lossy();
        assert!(
            (root.contains("concurrent-a") && data.metadata.project_name == "concurrent-a")
                || (root.contains("concurrent-b") && data.metadata.project_name == "concurrent-b")
        );

        let _ = std::fs::remove_dir_all(initial_root);
        let _ = std::fs::remove_dir_all(root_a);
        let _ = std::fs::remove_dir_all(root_b);
    }

    #[test]
    fn new_runtime_admission_stays_closed_until_publication_completes() {
        let (old_root, _) = save_named_project("runtime-admission-old");
        let (new_root, _) = save_named_project("runtime-admission-new");
        let state = ProjectState::new();
        state.activate_project_from_path(&old_root).unwrap();
        let prepared = state.prepare_project_activation(Some(&new_root)).unwrap();
        let old_runtime_session = state
            .project_store
            .read()
            .unwrap()
            .project_session_id
            .clone();
        let hook_state = state.clone();
        let observed = Arc::new(AtomicBool::new(false));
        let observed_for_hook = Arc::clone(&observed);
        state.set_activation_store_replaced_test_hook(Arc::new(move || {
            assert_eq!(hook_state.try_current_pre_run_admission_for_test(), None);
            assert_eq!(hook_state.try_current_run_admission_for_test(), None);
            observed_for_hook.store(true, Ordering::Release);
        }));

        state.activate_prepared_project(prepared).unwrap();

        assert!(observed.load(Ordering::Acquire));
        assert_eq!(state.try_current_pre_run_admission_for_test(), Some(true));
        assert_eq!(state.try_current_run_admission_for_test(), Some(true));
        let (runtime_session, identity_session) = state.runtime_identity_sessions_for_test();
        assert_eq!(runtime_session, identity_session);
        assert_ne!(runtime_session, old_runtime_session);

        let _ = std::fs::remove_dir_all(old_root);
        let _ = std::fs::remove_dir_all(new_root);
    }

    #[test]
    fn failed_preparation_leaves_current_identity_path_data_lifecycle_and_runtime_unchanged() {
        let (current_root, _) = save_named_project("preparation-current");
        let broken_root = project_root("preparation-broken");
        std::fs::write(
            broken_root.join(crate::project::PROJECT_METADATA_FILE),
            b"not json",
        )
        .unwrap();
        let state = ProjectState::new();
        state.activate_project_from_path(&current_root).unwrap();
        let graph_path = GraphResourcePath::new("events/Owned.yssbi-event").unwrap();
        let mut disk = ProjectData::new();
        disk.graphs.insert(
            graph_path.clone(),
            crate::project::GraphResourceDocument::new("Owned", GraphDocumentKind::Event),
        );
        crate::project::fixtures::write_graph(
            &disk,
            current_root.to_string_lossy().as_ref(),
            &graph_path,
        )
        .unwrap();
        let session = state.capture_project_session().unwrap();
        state
            .load_graph_projection(&session.instance_id, &graph_path, 1, "en-US")
            .unwrap();
        state.append_history_head_for_test();

        let before_instance = state.project_instance_id();
        let before_path = state.get_path();
        let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
        let before_history = state.history_status();
        let before_lifecycle = state.graph_lifecycle_entry_count();
        let before_revisions = state.revision_state_for_test();
        let before_generation = state.activation_generation_for_test();
        let before_recovery = state.project_recovery_marker().error();
        let (before_runs, before_session_id) = {
            let store = state.project_store.read().unwrap();
            (Arc::clone(&store.runs), store.project_session_id.clone())
        };

        assert!(
            state
                .prepare_project_activation(Some(&broken_root))
                .is_err()
        );

        assert_eq!(state.project_instance_id(), before_instance);
        assert_eq!(state.get_path(), before_path);
        assert_eq!(
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            before_data
        );
        assert_eq!(state.history_status(), before_history);
        assert_eq!(state.graph_lifecycle_entry_count(), before_lifecycle);
        assert_eq!(state.revision_state_for_test(), before_revisions);
        assert_eq!(state.activation_generation_for_test(), before_generation);
        assert_eq!(state.project_recovery_marker().error(), before_recovery);
        let store = state.project_store.read().unwrap();
        assert!(Arc::ptr_eq(&store.runs, &before_runs));
        assert_eq!(store.project_session_id, before_session_id);

        let _ = std::fs::remove_dir_all(current_root);
        let _ = std::fs::remove_dir_all(broken_root);
    }

    #[test]
    fn old_activation_authority_is_dropped_after_publication_guards_and_root_lease() {
        let (old_root, _) = save_named_project("drop-old");
        let (new_root, _) = save_named_project("drop-new");
        let state = ProjectState::new();
        state.activate_project_from_path(&old_root).unwrap();
        state.append_history_head_for_test();
        let prepared = state.prepare_project_activation(Some(&new_root)).unwrap();
        let normalized_new_root =
            crate::project::NormalizedProjectRoot::from_project_path(&new_root).unwrap();
        let (drop_tx, drop_rx) = std::sync::mpsc::channel();
        let drop_state = state.clone();
        state
            .project_store
            .write()
            .unwrap()
            .set_drop_test_hook(Arc::new(move || {
                drop_tx
                    .send((
                        drop_state.activation_publication_guards_are_available_for_test(),
                        !drop_state
                            .filesystem()
                            .is_reserved_for_test(&normalized_new_root),
                        drop_state.try_current_pre_run_admission_for_test(),
                        drop_state.try_current_run_admission_for_test(),
                    ))
                    .unwrap();
            }));

        state.activate_prepared_project(prepared).unwrap();

        let (guards_available, root_available, pre_run, run) =
            drop_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(guards_available);
        assert!(root_available);
        assert_eq!(pre_run, Some(true));
        assert_eq!(run, Some(true));

        let _ = std::fs::remove_dir_all(old_root);
        let _ = std::fs::remove_dir_all(new_root);
    }

    #[test]
    fn publication_panic_restores_even_generation_and_preserves_complete_session() {
        let (old_root, _) = save_named_project("panic-old");
        let (new_root, _) = save_named_project("panic-new");
        let state = ProjectState::new();
        let old_session = state.activate_project_from_path(&old_root).unwrap();
        state.append_history_head_for_test();
        let before_path = state.get_path();
        let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
        let before_history = state.history_status();
        let before_lifecycle = state.graph_lifecycle_entry_count();
        let before_revisions = state.revision_state_for_test();
        let before_recovery = state.project_recovery_marker().error();
        let (before_runs, before_runtime_session) = {
            let store = state.project_store.read().unwrap();
            (Arc::clone(&store.runs), store.project_session_id.clone())
        };
        let prepared = state.prepare_project_activation(Some(&new_root)).unwrap();
        state.set_activation_publication_panic_test_hook(Arc::new(|| {
            panic!("injected activation publication panic")
        }));

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = state.activate_prepared_project(prepared);
        }));

        assert!(panic.is_err());
        assert_eq!(state.activation_generation_for_test() % 2, 0);
        assert_eq!(state.capture_project_session().unwrap(), old_session);
        assert_eq!(state.get_path(), before_path);
        assert_eq!(
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            before_data
        );
        assert_eq!(state.history_status(), before_history);
        assert_eq!(state.graph_lifecycle_entry_count(), before_lifecycle);
        assert_eq!(state.revision_state_for_test(), before_revisions);
        assert_eq!(state.project_recovery_marker().error(), before_recovery);
        let store = state.project_store.read().unwrap();
        assert!(Arc::ptr_eq(&store.runs, &before_runs));
        assert_eq!(store.project_session_id, before_runtime_session);
        drop(store);
        assert_eq!(state.try_current_pre_run_admission_for_test(), Some(true));
        assert_eq!(state.try_current_run_admission_for_test(), Some(true));

        let reader_state = state.clone();
        let (reader_tx, reader_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            reader_tx
                .send(reader_state.capture_projection_environment_for_test())
                .unwrap();
        });
        assert!(
            reader_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_ok()
        );

        state.set_activation_publication_panic_test_hook(Arc::new(|| {}));
        state.activate_project_from_path(&new_root).unwrap();

        let _ = std::fs::remove_dir_all(old_root);
        let _ = std::fs::remove_dir_all(new_root);
    }

    #[test]
    fn same_root_prepared_activation_rejects_persisted_global_variable() {
        let (root, _) = save_named_project("same-root-global-variable");
        let state = ProjectState::new();
        let session = state.activate_project_from_path(&root).unwrap();
        let prepared = state.prepare_project_activation(Some(&root)).unwrap();
        let variable = state
            .add_variable(
                "committed",
                DataType::Int64,
                DataValue::Int64(42),
                "",
                VariableScope::Global,
                vec![],
            )
            .unwrap();
        crate::project::fixtures::flush_state(&state).unwrap();

        let error = state.activate_prepared_project(prepared).unwrap_err();

        assert_eq!(error.code(), "stale_project_lifecycle");
        assert_eq!(state.capture_project_session().unwrap(), session);
        assert!(
            state
                .get_data()
                .unwrap()
                .variables
                .contains_key(&variable.id)
        );
        assert!(
            load_project_from_file(root.to_string_lossy().as_ref())
                .unwrap()
                .variables
                .contains_key(&variable.id)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn same_root_prepared_activation_rejects_imported_database() {
        let (root, _) = save_named_project("same-root-database-import");
        let csv = root.join("import.csv");
        std::fs::write(&csv, "value\n1\n").unwrap();
        let state = ProjectState::new();
        let session = state.activate_project_from_path(&root).unwrap();
        let prepared = state.prepare_project_activation(Some(&root)).unwrap();
        let imported = crate::application::database::load_database(
            &state,
            &session.instance_id,
            crate::node_system::document::OperationId::new(),
            crate::schema::DatabaseEngineDTO::Csv {
                path: csv.to_string_lossy().into_owned(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: None,
            },
        )
        .unwrap()
        .data;

        let error = state.activate_prepared_project(prepared).unwrap_err();

        assert_eq!(error.code(), "stale_project_lifecycle");
        assert_eq!(state.capture_project_session().unwrap(), session);
        assert!(
            state
                .get_data()
                .unwrap()
                .databases
                .contains_key(&imported.id)
        );
        assert!(
            state
                .project_store
                .read()
                .unwrap()
                .databases
                .contains_key(&imported.id)
        );
        assert!(
            load_project_from_file(root.to_string_lossy().as_ref())
                .unwrap()
                .databases
                .contains_key(&imported.id)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn final_root_lease_blocks_database_import_until_activation_publishes() {
        let (root, _) = save_named_project("final-lease-database-import");
        let csv = root.join("blocked-import.csv");
        std::fs::write(&csv, "value\n1\n").unwrap();
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        let writer_project_id = state.capture_project_session().unwrap().instance_id;
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

        let writer_state = state.clone();
        let csv_path = csv.to_string_lossy().into_owned();
        let (writer_tx, writer_rx) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            let result = crate::application::database::load_database(
                &writer_state,
                &writer_project_id,
                crate::node_system::document::OperationId::new(),
                crate::schema::DatabaseEngineDTO::Csv {
                    path: csv_path,
                    delimiter: ',',
                    has_header: true,
                    infer_schema_length: None,
                },
            );
            writer_tx.send(()).unwrap();
            result
        });

        assert!(writer_rx.recv_timeout(Duration::from_millis(100)).is_err());
        release.wait();
        activation.join().unwrap().unwrap();
        writer_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let error = writer.join().unwrap().unwrap_err();
        assert!(error.contains("stale_project_lifecycle"));
        assert!(state.get_data().unwrap().databases.is_empty());
        assert!(state.project_store.read().unwrap().databases.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn final_rebuild_publishes_fresh_duckdb_runtime_metadata() {
        let (root, _) = save_named_project("final-rebuild-database-metadata");
        let initial_csv = root.join("initial.csv");
        std::fs::write(&initial_csv, "value\n1\n").unwrap();
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let imported = crate::application::database::load_database(
            &state,
            &project_instance_id,
            crate::node_system::document::OperationId::new(),
            crate::schema::DatabaseEngineDTO::Csv {
                path: initial_csv.to_string_lossy().into_owned(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: None,
            },
        )
        .unwrap()
        .data;
        let prepared = state.prepare_project_activation(Some(&root)).unwrap();
        let declaration = state
            .get_data()
            .unwrap()
            .databases
            .get(&imported.id)
            .unwrap()
            .clone();
        let (relative_path, table) = declaration.engine.duckdb_table().unwrap();
        let changed_csv = root.join("changed.csv");
        std::fs::write(&changed_csv, "value,extra\n1,a\n2,b\n").unwrap();
        crate::database::ingest_csv_to_duckdb(
            &changed_csv,
            &root.join(relative_path),
            table,
            ',',
            true,
            None,
        )
        .unwrap();

        state.activate_prepared_project(prepared).unwrap();

        let metadata =
            crate::application::database::get_database_meta(&state, &imported.id).unwrap();
        assert_eq!(metadata.row_count, 2);
        assert_eq!(metadata.column_count, 2);
        assert!(metadata.columns.iter().any(|column| column.name == "extra"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn same_root_prepared_activation_rebuilds_authority_changing_flush() {
        let (root, _) = save_named_project("same-root-authority-flush");
        let state = ProjectState::new();
        let session = state.activate_project_from_path(&root).unwrap();
        let prepared = state.prepare_project_activation(Some(&root)).unwrap();
        state.project_data.write().unwrap().metadata.project_name = "committed name".into();
        crate::project::fixtures::flush_state(&state).unwrap();

        let replacement = state.activate_prepared_project(prepared).unwrap();

        assert_ne!(replacement.instance_id, session.instance_id);
        assert_eq!(replacement.root, session.root);
        assert_eq!(
            state.get_data().unwrap().metadata.project_name,
            "committed name"
        );
        assert_eq!(
            load_project_from_file(root.to_string_lossy().as_ref())
                .unwrap()
                .metadata
                .project_name,
            "committed name"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn same_root_prepared_activation_accepts_semantically_equivalent_flush() {
        let (root, _) = save_named_project("same-root-equivalent-flush");
        let state = ProjectState::new();
        let old_session = state.activate_project_from_path(&root).unwrap();
        let prepared = state.prepare_project_activation(Some(&root)).unwrap();
        crate::project::fixtures::flush_state(&state).unwrap();

        let replacement = state.activate_prepared_project(prepared).unwrap();

        assert_ne!(replacement.instance_id, old_session.instance_id);
        assert_eq!(replacement.root, old_session.root);
        assert_eq!(
            state.get_data().unwrap().metadata.project_name,
            "same-root-equivalent-flush"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preparation_rejects_authority_change_during_leased_disk_read() {
        let (root, _) = save_named_project("preparation-authority-race");
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        let hook_state = state.clone();
        state.set_activation_preparation_after_read_test_hook(Arc::new(move || {
            hook_state
                .add_variable(
                    "raced",
                    DataType::Int64,
                    DataValue::Int64(7),
                    "",
                    VariableScope::Global,
                    vec![],
                )
                .unwrap();
        }));

        let error = match state.prepare_project_activation(Some(&root)) {
            Ok(_) => panic!("authority-changing preparation must be rejected"),
            Err(error) => error,
        };

        assert_eq!(error.code(), "stale_project_lifecycle");
        assert!(
            state
                .get_data()
                .unwrap()
                .variables
                .values()
                .any(|variable| variable.name == "raced")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn successful_activation_clears_recovered_publication_lock_poison() {
        let (old_root, _) = save_named_project("poison-old");
        let (new_root, _) = save_named_project("poison-new");
        let state = ProjectState::new();
        state.activate_project_from_path(&old_root).unwrap();
        let prepared = state.prepare_project_activation(Some(&new_root)).unwrap();
        let poison_state = state.clone();
        assert!(
            std::thread::spawn(move || poison_state.poison_project_path_for_test())
                .join()
                .is_err()
        );

        let session = state.activate_prepared_project(prepared).unwrap();

        assert_eq!(
            state.get_path().as_deref(),
            Some(session.root.as_path().to_string_lossy().as_ref())
        );
        assert_eq!(
            state.get_data().unwrap().metadata.project_name,
            "poison-new"
        );
        assert_eq!(state.capture_project_session().unwrap(), session);
        assert_eq!(state.activation_generation_for_test() % 2, 0);
        assert_eq!(state.try_current_pre_run_admission_for_test(), Some(true));
        assert_eq!(state.try_current_run_admission_for_test(), Some(true));

        let _ = std::fs::remove_dir_all(old_root);
        let _ = std::fs::remove_dir_all(new_root);
    }

    #[test]
    fn central_authority_generation_tracks_global_database_and_legacy_commits() {
        let (root, _) = save_named_project("central-authority-generation");
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        let initial = state.authority_generation_for_test();

        state
            .add_variable(
                "generation",
                DataType::Int64,
                DataValue::Int64(1),
                "",
                VariableScope::Global,
                vec![],
            )
            .unwrap();
        let after_global = state.authority_generation_for_test();
        assert!(after_global > initial);

        crate::project::fixtures::flush_state(&state).unwrap();
        assert_eq!(state.authority_generation_for_test(), after_global);

        let graph_path = GraphResourcePath::new("events/Generation.yssbi-event").unwrap();
        state
            .insert_graph(
                graph_path,
                crate::project::GraphResourceDocument::new("Generation", GraphDocumentKind::Event),
            )
            .unwrap();
        let after_legacy = state.authority_generation_for_test();
        assert!(after_legacy > after_global);

        let database_id = "generation-db".to_string();
        let (database_session, _lease) = state.acquire_database_write_lease().unwrap();
        state
            .add_database_for_session(
                &database_session,
                &database_session.instance_id,
                crate::node_system::document::OperationId::new(),
                crate::database::DatabaseInstance {
                    decl: crate::database::DatabaseDecl {
                        id: database_id.clone(),
                        engine: crate::database::DatabaseEngine::InMemory {
                            name: "generation".into(),
                        },
                        schema_version: 1,
                        required: false,
                        name: Some("generation".into()),
                    },
                    state: crate::database::DatabaseState::Failed {
                        error: "test fixture".into(),
                    },
                },
            )
            .unwrap();
        assert!(state.authority_generation_for_test() > after_legacy);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn same_root_prepared_activation_rejects_committed_resource_transaction() {
        let (root, _) = save_named_project("same-root-stale-prepared");
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        let prepared = state.prepare_project_activation(Some(&root)).unwrap();
        let session = state.capture_project_session().unwrap();
        let graph_path = GraphResourcePath::new("events/Committed.yssbi-event").unwrap();
        let resource =
            crate::project::GraphResourceDocument::new("Committed", GraphDocumentKind::Event);
        let context = crate::project::ProjectTransactionContext {
            session: session.clone(),
            operation_id: crate::node_system::document::OperationId::new(),
            affected_resources: Vec::new(),
            expected_revisions: Default::default(),
            expected_absent_resources: [crate::node_system::document::ResourceKey::Graph(
                crate::node_system::document::GraphResourcePath(graph_path.as_str().into()),
            )]
            .into_iter()
            .collect(),
            recovery_marker: Some(state.project_recovery_marker()),
        };
        let contents = crate::project::project_io::serialize_graph_resource_document(
            &resource,
            std::collections::HashMap::new(),
        )
        .unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let transaction = crate::project::ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            lease,
            vec![crate::project::StagedFilesystemMutation::Write {
                relative_path: graph_path.as_str().into(),
                contents,
            }],
            |_, contents| {
                serde_json::from_slice::<crate::project::project_io::GraphDocument>(contents)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            },
        )
        .unwrap();
        let committed = transaction.commit().unwrap();
        state
            .apply_resource_document_patch(
                &context,
                crate::project::ResourceDocumentPatch::InsertGraph {
                    path: graph_path.clone(),
                    resource: resource.clone(),
                },
            )
            .unwrap();
        committed.finalize();

        let error = state.activate_prepared_project(prepared).unwrap_err();

        assert_eq!(error.code(), "stale_project_lifecycle");
        assert_eq!(state.capture_project_session().unwrap(), session);
        assert_eq!(
            state.get_data().unwrap().graphs.get(&graph_path),
            Some(&resource)
        );
        assert!(root.join(graph_path.as_str()).is_file());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn same_root_reactivation_invalidates_old_graph_lifecycle_owners() {
        let (root, _) = save_named_project("same-root");
        let graph_path = GraphResourcePath::new("events/Owned.yssbi-event").unwrap();
        let mut disk = ProjectData::new();
        disk.graphs.insert(
            graph_path.clone(),
            crate::project::GraphResourceDocument::new("Owned", GraphDocumentKind::Event),
        );
        crate::project::fixtures::write_graph(&disk, root.to_string_lossy().as_ref(), &graph_path)
            .unwrap();
        let state = ProjectState::new();
        let old_session = state.activate_project_from_path(&root).unwrap();
        state
            .load_graph_projection(&old_session.instance_id, &graph_path, 1, "en-US")
            .unwrap();
        assert_eq!(state.graph_lifecycle_entry_count(), 1);

        let replacement = state.activate_project_from_path(&root).unwrap();

        assert_ne!(replacement.instance_id, old_session.instance_id);
        assert_eq!(replacement.root, old_session.root);
        assert_eq!(state.graph_lifecycle_entry_count(), 0);
        assert_eq!(
            state
                .validate_project_session(&old_session)
                .unwrap_err()
                .code(),
            "stale_project_lifecycle"
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
