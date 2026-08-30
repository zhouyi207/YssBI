use super::*;
use crate::project::{ProjectState, fixtures};
use sqlx::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use yss_data_contract::{DataType, DataValue};
use yss_project_filesystem::ProjectRootBinding;
use yss_project_model::ProjectData;
use yss_project_registry::ProjectRegistry;
use yss_project_registry_contract::{
    ProjectRecord, ProjectRegistryStore, ProjectRegistryStoreError, ProjectRegistryStoreFuture,
    ProjectRootIdentityState,
};
use yss_variable_contract::VariableScope;

static DELETE_TEST_LOCK: Mutex<()> = Mutex::new(());

struct FailingRemoveProjectRegistryStore {
    inner: yss_project_registry_sqlite::SqliteProjectRegistryStore,
}

impl ProjectRegistryStore for FailingRemoveProjectRegistryStore {
    fn load(
        &self,
    ) -> ProjectRegistryStoreFuture<'_, Result<Box<[ProjectRecord]>, ProjectRegistryStoreError>>
    {
        self.inner.load()
    }

    fn upsert(
        &self,
        record: &ProjectRecord,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>> {
        self.inner.upsert(record)
    }

    fn remove(
        &self,
        _registration: &yss_project_identity::ProjectRegistrationId,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>> {
        Box::pin(async { Err(ProjectRegistryStoreError::StorageFailed) })
    }
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

struct DeleteHookReset;

impl Drop for DeleteHookReset {
    fn drop(&mut self) {
        set_before_registry_remove_test_hook(None);
        crate::project::set_recycle_bin_test_hook(None);
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
    let path = store.path().to_path_buf();
    ProjectRegistry::new(Arc::new(store), path)
}

async fn initialize_registry_with_remove_failure(directory: &TestDirectory) -> ProjectRegistry {
    let store = yss_project_registry_sqlite::SqliteProjectRegistryStore::connect(
        directory.child("registry"),
    )
    .await
    .unwrap();
    let path = store.path().to_path_buf();
    ProjectRegistry::new(
        Arc::new(FailingRemoveProjectRegistryStore { inner: store }),
        path,
    )
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

async fn mark_registry_record_invalid(registry: &ProjectRegistry, id: &str) {
    let url = format!(
        "sqlite://{}",
        registry.path().to_string_lossy().replace('\\', "/")
    );
    let mut connection = sqlx::SqliteConnection::connect(&url).await.unwrap();
    sqlx::query(
        "UPDATE projects SET root_identity = '', root_identity_state = 'invalid' WHERE id = ?",
    )
    .bind(id)
    .execute(&mut connection)
    .await
    .unwrap();
}

#[test]
fn save_as_registry_failure_preserves_source_and_disk_with_exact_receipt() {
    tauri::async_runtime::block_on(async {
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
    tauri::async_runtime::block_on(async {
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
                        .add_variable(
                            "authority drift",
                            DataType::Int64,
                            DataValue::Int64(1),
                            "",
                            VariableScope::Global,
                            Vec::new(),
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
fn registry_failure_reports_preserved_created_project() {
    tauri::async_runtime::block_on(async {
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

#[test]
fn recycle_bin_failure_leaves_project_and_registry_for_retry() {
    let _serial = DELETE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _reset = DeleteHookReset;
    tauri::async_runtime::block_on(async {
        let directory = TestDirectory::new("delete-application-replacement-failure");
        let registry = initialize_registry(&directory).await;
        let root = directory.child("project");
        let state = activate_named_project(&root, "Registered");
        let session = state.capture_project_session().unwrap();
        let record = register_root(&registry, &root, "Registered").await;
        let session_before = session.clone();
        let operation_id = OperationId::new();
        crate::project::set_recycle_bin_test_hook(Some(Arc::new(|_| {
            Err("injected recycle-bin failure".into())
        })));

        let error = delete_registered_project(
            &state,
            &registry,
            record.id.as_str(),
            Some(session.instance_id),
            operation_id,
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            ProjectLifecycleError::AuthorityFailed(error)
                if error.code() == "transaction_commit_failed"
        ));
        assert_eq!(state.capture_project_session().unwrap(), session_before);
        assert!(
            root.join(yss_project_layout::PROJECT_METADATA_FILE)
                .is_file()
        );
        assert!(
            registry
                .fetch_by_id(record.id.as_str())
                .await
                .unwrap()
                .is_some()
        );
    });
}

#[test]
fn registered_deletion_returns_committed_receipt_after_recycle_bin_move() {
    let _serial = DELETE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _reset = DeleteHookReset;
    tauri::async_runtime::block_on(async {
        let directory = TestDirectory::new("delete-application-success");
        let registry = initialize_registry(&directory).await;
        let root = directory.child("project");
        let state = activate_named_project(&root, "Registered");
        let session = state.capture_project_session().unwrap();
        let record = register_root(&registry, &root, "Registered").await;
        let normalized = ProjectRootBinding::for_existing(&root)
            .unwrap()
            .normalized()
            .clone();
        let operation_id = OperationId::new();
        let expected = ProjectLifecycleApplicationEvent {
            operation_id,
            kind: ProjectLifecycleKind::Delete,
            old_project_instance_id: Some(session.instance_id.clone()),
            new_project_instance_id: None,
            phase: ProjectLifecyclePhase::AuthorityCommitted,
            outcome: ProjectLifecycleOutcome::Committed,
            record: Some(record.clone()),
            path: Some(
                normalized
                    .as_path()
                    .to_string_lossy()
                    .into_owned()
                    .into_boxed_str(),
            ),
            recovery: None,
            invalidation: LifecycleInvalidation {
                project: true,
                registry: true,
            },
        };

        let result = delete_registered_project(
            &state,
            &registry,
            record.id.as_str(),
            Some(session.instance_id),
            operation_id,
        )
        .await
        .unwrap();

        assert_eq!(result, expected);
        assert!(state.capture_project_session().is_err());
        assert!(!root.exists());
        assert!(
            registry
                .fetch_by_id(record.id.as_str())
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            state.filesystem().lifecycle_state_for_test(&normalized),
            (false, false, 0)
        );
    });
}

#[test]
fn registry_future_panic_returns_exact_pending_receipt_and_releases_ownership() {
    let _serial = DELETE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _reset = DeleteHookReset;
    tauri::async_runtime::block_on(async {
        let directory = TestDirectory::new("delete-application-registry-panic");
        let registry = initialize_registry(&directory).await;
        let root = directory.child("project");
        let state = activate_named_project(&root, "Registered");
        let session = state.capture_project_session().unwrap();
        let record = register_root(&registry, &root, "Registered").await;
        let identity = record.deletion_identity().unwrap().clone();
        let operation_id = OperationId::new();
        let normalized = ProjectRootBinding::for_existing(&root)
            .unwrap()
            .normalized()
            .clone();
        let expected = ProjectLifecycleApplicationEvent {
            operation_id,
            kind: ProjectLifecycleKind::Delete,
            old_project_instance_id: Some(session.instance_id.clone()),
            new_project_instance_id: None,
            phase: ProjectLifecyclePhase::AuthorityCommitted,
            outcome: ProjectLifecycleOutcome::RegistryPending,
            record: Some(record.clone()),
            path: Some(
                normalized
                    .as_path()
                    .to_string_lossy()
                    .into_owned()
                    .into_boxed_str(),
            ),
            recovery: Some(LifecycleRecovery {
                required: true,
                action: LifecycleRecoveryAction::RemoveRegistryRecord,
                path: None,
                identity: Some(identity.as_str().to_owned().into_boxed_str()),
            }),
            invalidation: LifecycleInvalidation {
                project: true,
                registry: true,
            },
        };
        let filesystem = state.filesystem().clone();
        let root_during_registry = normalized.clone();
        set_before_registry_remove_test_hook(Some(Arc::new(move || {
            assert_eq!(
                filesystem.lifecycle_state_for_test(&root_during_registry),
                (false, false, 0)
            );
            panic!("injected registry future panic")
        })));

        let result = delete_registered_project(
            &state,
            &registry,
            record.id.as_str(),
            Some(session.instance_id),
            operation_id,
        )
        .await
        .unwrap();

        assert_eq!(result, expected);
        assert!(state.capture_project_session().is_err());
        assert_eq!(
            state.filesystem().lifecycle_state_for_test(&normalized),
            (false, false, 0)
        );
        drop(state.filesystem().acquire(normalized).unwrap());
        assert!(
            registry
                .fetch_by_id(record.id.as_str())
                .await
                .unwrap()
                .is_some()
        );
    });
}

#[test]
fn invalid_active_row_is_rejected_as_registry_cleanup_without_file_deletion() {
    let _serial = DELETE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _reset = DeleteHookReset;
    tauri::async_runtime::block_on(async {
        let directory = TestDirectory::new("delete-application-invalid-active");
        let registry = initialize_registry(&directory).await;
        let root = directory.child("project");
        let state = activate_named_project(&root, "Invalid");
        std::fs::write(root.join("sentinel.txt"), b"preserve").unwrap();
        let session = state.capture_project_session().unwrap();
        let registered = register_root(&registry, &root, "Invalid").await;
        mark_registry_record_invalid(&registry, registered.id.as_str()).await;
        let record = registry
            .fetch_by_id(registered.id.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            record.root_identity_state,
            ProjectRootIdentityState::Invalid
        );
        let operation_id = OperationId::new();
        let expected = ProjectLifecycleApplicationEvent {
            operation_id,
            kind: ProjectLifecycleKind::RegistryCleanup,
            old_project_instance_id: None,
            new_project_instance_id: None,
            phase: ProjectLifecyclePhase::RegistryCommitted,
            outcome: ProjectLifecycleOutcome::RegistryFailed,
            record: Some(record.clone()),
            path: None,
            recovery: Some(LifecycleRecovery {
                required: true,
                action: LifecycleRecoveryAction::CleanupRegistry,
                path: None,
                identity: None,
            }),
            invalidation: LifecycleInvalidation {
                project: false,
                registry: true,
            },
        };

        let result = delete_registered_project(
            &state,
            &registry,
            record.id.as_str(),
            Some(session.instance_id.clone()),
            operation_id,
        )
        .await
        .unwrap();

        assert_eq!(result, expected);
        assert_eq!(state.capture_project_session().unwrap(), session);
        assert_eq!(
            std::fs::read(root.join("sentinel.txt")).unwrap(),
            b"preserve"
        );
        assert!(
            registry
                .fetch_by_id(record.id.as_str())
                .await
                .unwrap()
                .is_some()
        );
    });
}

#[test]
fn registry_failure_commits_clear_and_returns_registry_pending_with_released_ownership() {
    let _serial = DELETE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _reset = DeleteHookReset;
    tauri::async_runtime::block_on(async {
        let directory = TestDirectory::new("delete-application-registry-failure");
        let registry = initialize_registry_with_remove_failure(&directory).await;
        let root = directory.child("project");
        let state = activate_named_project(&root, "Registered");
        let session = state.capture_project_session().unwrap();
        let record = register_root(&registry, &root, "Registered").await;
        let normalized = ProjectRootBinding::for_existing(&root)
            .unwrap()
            .normalized()
            .clone();
        let result = delete_registered_project(
            &state,
            &registry,
            record.id.as_str(),
            Some(session.instance_id),
            OperationId::new(),
        )
        .await
        .unwrap();

        assert_eq!(result.outcome, ProjectLifecycleOutcome::RegistryPending);
        assert_eq!(
            result.recovery.as_ref().unwrap().action,
            LifecycleRecoveryAction::RemoveRegistryRecord
        );
        assert!(!root.exists());
        assert!(state.capture_project_session().is_err());
        assert_eq!(
            state.filesystem().lifecycle_state_for_test(&normalized),
            (false, false, 0)
        );
        drop(state.filesystem().acquire(normalized).unwrap());
        assert!(
            registry
                .fetch_by_id(record.id.as_str())
                .await
                .unwrap()
                .is_some()
        );
    });
}
