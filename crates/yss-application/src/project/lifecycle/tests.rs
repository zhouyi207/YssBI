use super::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use yss_project::{ProjectState, fixtures};
use yss_project_model::ProjectData;
use yss_project_registry::ProjectRegistry;
use yss_project_registry_contract::ProjectRecord;

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new()
        .expect("test runtime is available")
        .block_on(future)
}

struct TestDirectory {
    root: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn child(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write_named_project(root: &Path, name: &str) -> ProjectData {
    std::fs::create_dir_all(root).unwrap();
    let mut data = ProjectData::new();
    data.metadata.project_name = name.into();
    fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
    data
}

fn activate_named_project(root: &Path, name: &str) -> ProjectState {
    let data = write_named_project(root, name);
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
    state
}

async fn initialize_registry(directory: &TestDirectory) -> ProjectRegistry {
    let store = yss_project_registry_sqlite::SqliteProjectRegistryStore::connect(
        directory.child("registry"),
    )
    .await
    .unwrap();
    ProjectRegistry::new(Arc::new(store))
}

async fn register_root(registry: &ProjectRegistry, root: &Path, name: &str) -> ProjectRecord {
    registry
        .register_project(
            name,
            root.join(yss_project_layout::PROJECT_METADATA_FILE)
                .to_string_lossy()
                .as_ref(),
        )
        .await
        .unwrap()
}

async fn seed_stale_registration(registry: &ProjectRegistry, destination: &Path) -> ProjectRecord {
    write_named_project(destination, "Stale registration");
    let record = register_root(registry, destination, "Stale registration").await;
    std::fs::remove_dir_all(destination).unwrap();
    record
}

#[test]
fn late_watcher_lifecycle_tails_preserve_the_current_project_source() {
    use yss_filesystem::watcher::{
        ChangeSink, FileWatcherDrain, FileWatcherDrainOutcome, FileWatcherFactory,
        FileWatcherSession, FileWatcherStartError, ObservedChange, WatcherEpoch,
        WatcherShutdownControl, WatcherState,
    };

    #[derive(Default)]
    struct RecordingFactory(Arc<Mutex<Option<PathBuf>>>);

    struct RecordingSession(Arc<Mutex<Option<PathBuf>>>);

    impl FileWatcherFactory for RecordingFactory {
        fn start(
            &self,
            root: &Path,
            _epoch: WatcherEpoch,
            _sink: Arc<dyn ChangeSink>,
        ) -> Result<Box<dyn FileWatcherSession>, FileWatcherStartError> {
            *self.0.lock().unwrap() = Some(root.to_path_buf());
            Ok(Box::new(RecordingSession(self.0.clone())))
        }
    }

    impl FileWatcherSession for RecordingSession {
        fn close_admission(self: Box<Self>) -> Box<dyn FileWatcherDrain> {
            *self.0.lock().unwrap() = None;
            self
        }
    }

    impl FileWatcherDrain for RecordingSession {
        fn finish(self: Box<Self>, _control: WatcherShutdownControl) -> FileWatcherDrainOutcome {
            FileWatcherDrainOutcome::Drained
        }
    }

    struct NoopSink;

    impl ChangeSink for NoopSink {
        fn publish(&self, _change: ObservedChange) {}
    }

    let directory = TestDirectory::new("watcher-lifecycle-tails");
    let first_root = directory.child("first");
    let second_root = directory.child("second");
    write_named_project(&first_root, "First");
    write_named_project(&second_root, "Second");
    let application = ApplicationState::initialize().unwrap();
    let factory = Arc::new(RecordingFactory::default());
    let watcher = Mutex::new(WatcherState::new(factory.clone()));
    let watch = |activation: &ProjectActivation| {
        application
            .watch_project_changes(
                &watcher,
                &activation.path,
                &activation.project_instance_id,
                Arc::new(NoopSink),
            )
            .unwrap();
    };

    let first = application
        .load_project_for_application(first_root.to_str().unwrap())
        .unwrap();
    watch(&first);
    let second = application
        .load_project_for_application(second_root.to_str().unwrap())
        .unwrap();
    watch(&second);
    watch(&first);
    let late_start_preserved = factory.0.lock().unwrap().as_ref()
        == Some(&yss_project::project_root_from_path(&second.path));

    application
        .clear_project_for_application(&second.project_instance_id)
        .unwrap();
    let reopened = application
        .load_project_for_application(first_root.to_str().unwrap())
        .unwrap();
    watch(&reopened);
    application.stop_project_watcher(&watcher);
    let late_stop_preserved = factory.0.lock().unwrap().as_ref()
        == Some(&yss_project::project_root_from_path(&reopened.path));

    application
        .clear_project_for_application(&reopened.project_instance_id)
        .unwrap();
    application.stop_project_watcher(&watcher);
    assert!(factory.0.lock().unwrap().is_none());
    assert_eq!(
        (late_start_preserved, late_stop_preserved),
        (true, true),
        "late activation and close tails must leave the successor watcher intact"
    );
}

#[test]
fn closing_project_releases_its_session() {
    let directory = TestDirectory::new("close-project");
    let root = directory.child("project");
    write_named_project(&root, "Close me");
    let application = ApplicationState::initialize().unwrap();
    let activation = application
        .load_project_for_application(root.to_str().unwrap())
        .unwrap();
    let old = application.capture_session().unwrap();

    assert!(
        application
            .clear_project_for_application(&ProjectInstanceId::new())
            .is_err()
    );
    assert!(application.revalidate_captured_session(&old).is_ok());
    application
        .clear_project_for_application(&activation.project_instance_id)
        .unwrap();
    assert!(application.revalidate_captured_session(&old).is_err());
    drop(old);
    assert!(root.exists());
    let current = application.capture_session().unwrap();
    assert!(current.project().capture_project_session().is_err());
}

#[test]
fn save_as_registry_failure_preserves_source_and_disk_with_exact_receipt() {
    block_on(async {
        let directory = TestDirectory::new("save-as-application-registry-failure");
        let registry = initialize_registry(&directory).await;
        let source = directory.child("source");
        let destination = directory.child("destination");
        let state = activate_named_project(&source, "Workflow");
        let source_session = state.capture_project_session().unwrap();
        seed_stale_registration(&registry, &destination).await;
        let operation_id = OperationId::new();

        let result = save_project_as(
            &state,
            &registry,
            &destination,
            source_session.instance_id.clone(),
            operation_id,
        )
        .await
        .unwrap();

        assert_eq!(result.operation_id, operation_id);
        assert_eq!(result.kind, ProjectLifecycleKind::SaveAs);
        assert_eq!(result.phase, ProjectLifecyclePhase::DestinationCommitted);
        assert_eq!(result.outcome, ProjectLifecycleOutcome::RegistryFailed);
        assert_eq!(
            result.old_project_instance_id.as_ref(),
            Some(&source_session.instance_id)
        );
        assert!(result.new_project_instance_id.is_none());
        assert!(result.record.is_none());
        assert_eq!(
            result.recovery.as_ref().unwrap().action,
            LifecycleRecoveryAction::RegisterDestination
        );
        assert!(
            destination
                .join(yss_project_layout::PROJECT_METADATA_FILE)
                .is_file()
        );
        assert_eq!(state.capture_project_session().unwrap(), source_session);
        assert!(
            source
                .join(yss_project_layout::PROJECT_METADATA_FILE)
                .is_file()
        );
    });
}

#[test]
fn save_as_activation_failure_and_success_return_exact_direct_receipts() {
    block_on(async {
        for fail_activation in [true, false] {
            let directory = TestDirectory::new("save-as-application-activation");
            let registry = initialize_registry(&directory).await;
            let source = directory.child("source");
            let destination = directory.child("destination");
            let state = activate_named_project(&source, "Boundary");
            let source_session = state.capture_project_session().unwrap();
            if fail_activation {
                let hook_state = state.clone();
                state.set_project_activation_test_hook(Arc::new(move || {
                    hook_state
                        .create_graph_resource(
                            &hook_state.capture_project_session().unwrap().instance_id,
                            "authority drift",
                            yss_project_model::GraphResourceDocument::new(
                                "authority drift",
                                yss_graph_document::GraphResourceKind::EventGraph,
                            ),
                            OperationId::new(),
                        )
                        .unwrap();
                }));
            }
            let operation_id = OperationId::new();

            let result = save_project_as(
                &state,
                &registry,
                &destination,
                source_session.instance_id.clone(),
                operation_id,
            )
            .await
            .unwrap();

            assert_eq!(result.operation_id, operation_id);
            assert_eq!(result.kind, ProjectLifecycleKind::SaveAs);
            assert!(
                destination
                    .join(yss_project_layout::PROJECT_METADATA_FILE)
                    .is_file()
            );
            assert!(result.record.is_some());
            if fail_activation {
                assert_eq!(result.phase, ProjectLifecyclePhase::RegistryCommitted);
                assert_eq!(result.outcome, ProjectLifecycleOutcome::ActivationFailed);
                assert_eq!(
                    result.recovery.as_ref().unwrap().action,
                    LifecycleRecoveryAction::ActivateDestination
                );
                assert_eq!(state.capture_project_session().unwrap(), source_session);
            } else {
                assert_eq!(result.phase, ProjectLifecyclePhase::AuthorityCommitted);
                assert_eq!(result.outcome, ProjectLifecycleOutcome::Committed);
                assert!(result.recovery.is_none());
                assert_ne!(state.capture_project_session().unwrap(), source_session);
            }
        }
    });
}

#[test]
fn created_project_name_agrees_with_manifest_and_registry() {
    block_on(async {
        let directory = TestDirectory::new("create-project-names");
        let registry = initialize_registry(&directory).await;
        let state = ProjectState::new();
        for (folder, input, expected) in [
            ("trimmed", "  Example  ", "Example"),
            ("blank", " \t ", yss_project_model::DEFAULT_PROJECT_NAME),
        ] {
            let destination = directory.child(folder);
            let result = create_project(&state, &registry, input, &destination, OperationId::new())
                .await
                .unwrap();
            assert_eq!(result.outcome, ProjectLifecycleOutcome::Committed);
            let record = result.record.unwrap();
            let manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(destination.join(yss_project_layout::PROJECT_METADATA_FILE))
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(manifest["projectName"], expected);
            assert_eq!(record.name, expected);
            assert_eq!(
                registry
                    .fetch_by_id(record.id.as_str())
                    .await
                    .unwrap()
                    .unwrap()
                    .name,
                expected
            );
        }
    });
}

#[test]
fn registry_failure_reports_preserved_created_project() {
    block_on(async {
        let directory = TestDirectory::new("create-application-registry-failure");
        let registry = initialize_registry(&directory).await;
        let destination = directory.child("destination");
        seed_stale_registration(&registry, &destination).await;
        let state = ProjectState::new();
        let operation_id = OperationId::new();

        let result = create_project(&state, &registry, "Preserved", &destination, operation_id)
            .await
            .unwrap();

        assert_eq!(result.operation_id, operation_id);
        assert_eq!(result.kind, ProjectLifecycleKind::Create);
        assert_eq!(result.phase, ProjectLifecyclePhase::DestinationCommitted);
        assert_eq!(result.outcome, ProjectLifecycleOutcome::RegistryFailed);
        assert!(result.record.is_none());
        assert_eq!(
            result.recovery.as_ref().unwrap().action,
            LifecycleRecoveryAction::RegisterDestination
        );
        assert!(
            destination
                .join(yss_project_layout::PROJECT_METADATA_FILE)
                .is_file()
        );
        assert!(destination.join(yss_project_layout::DATABASE_DIR).is_dir());
    });
}
