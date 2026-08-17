use crate::error::CommandError;
use crate::event::{
    LifecycleInvalidationDto, LifecycleMutationKindDto, LifecycleMutationOutcomeDto,
    LifecycleMutationPhaseDto, LifecycleMutationResultDto, LifecycleRecoveryDto,
};
use crate::node_system::document::OperationId;
use crate::project::{
    CleanupInvalidProjectsResult, ProjectInstanceId, ProjectPickerTaskCancelRegistry,
    ProjectRecord, ProjectRegistry, ScanProjectsResult,
};
use std::future::Future;
use tauri::{State, ipc::Channel};

async fn catch_future_unwind<F>(future: F) -> Result<F::Output, Box<dyn std::any::Any + Send>>
where
    F: Future,
{
    let mut future = Box::pin(future);
    std::future::poll_fn(|context| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            future.as_mut().poll(context)
        })) {
            Ok(std::task::Poll::Ready(output)) => std::task::Poll::Ready(Ok(output)),
            Ok(std::task::Poll::Pending) => std::task::Poll::Pending,
            Err(payload) => std::task::Poll::Ready(Err(payload)),
        }
    })
    .await
}

#[tauri::command]
pub async fn list_registered_projects(
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<ProjectRecord>, CommandError> {
    registry
        .list_projects()
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub async fn scan_projects_in_directory(
    registry: State<'_, ProjectRegistry>,
    task_cancel: State<'_, ProjectPickerTaskCancelRegistry>,
    directory: String,
    on_progress: Channel<crate::project::ProjectScanProgressEvent>,
) -> Result<ScanProjectsResult, CommandError> {
    let cancel = task_cancel.begin();
    let result = registry
        .scan_directory(&directory, Some(on_progress), cancel.clone())
        .await;
    task_cancel.end(&cancel);
    result.map_err(CommandError::internal)
}

#[tauri::command]
pub fn cancel_project_picker_task(task_cancel: State<'_, ProjectPickerTaskCancelRegistry>) {
    task_cancel.cancel_active();
}

#[tauri::command]
pub async fn cleanup_invalid_registered_projects(
    registry: State<'_, ProjectRegistry>,
    task_cancel: State<'_, ProjectPickerTaskCancelRegistry>,
    on_progress: Channel<crate::project::ProjectCleanupProgressEvent>,
) -> Result<CleanupInvalidProjectsResult, CommandError> {
    let cancel = task_cancel.begin();
    let result = registry
        .cleanup_invalid_projects(Some(on_progress), cancel.clone())
        .await;
    task_cancel.end(&cancel);
    result.map_err(CommandError::internal)
}

#[tauri::command]
pub async fn register_project(
    registry: State<'_, ProjectRegistry>,
    name: String,
    path: String,
) -> Result<ProjectRecord, CommandError> {
    registry
        .register_project(&name, &path)
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub async fn remove_registered_project(
    registry: State<'_, ProjectRegistry>,
    id: String,
) -> Result<(), CommandError> {
    registry
        .remove_project(&id)
        .await
        .map_err(CommandError::internal)
}

async fn delete_registered_project_workflow<Remove, RemoveFuture, Emit>(
    state: &crate::project::ProjectState,
    record: ProjectRecord,
    expected_active_instance_id: Option<ProjectInstanceId>,
    operation_id: OperationId,
    remove: Remove,
    emit: Emit,
) -> Result<LifecycleMutationResultDto, CommandError>
where
    Remove: FnOnce() -> RemoveFuture,
    RemoveFuture: Future<Output = Result<(), String>>,
    Emit: FnOnce(&crate::event::Event) -> Result<(), String>,
{
    let Some(identity) = record.deletion_identity().cloned() else {
        let active_delete_rejected = expected_active_instance_id.is_some();
        let remove_result = if active_delete_rejected {
            Err("registry-only cleanup cannot delete an active project".into())
        } else {
            remove().await
        };
        let registry_changed = remove_result.is_ok();
        let result = LifecycleMutationResultDto {
            operation_id,
            kind: LifecycleMutationKindDto::RegistryCleanup,
            old_project_instance_id: None,
            new_project_instance_id: None,
            phase: LifecycleMutationPhaseDto::RegistryCommitted,
            outcome: if registry_changed {
                LifecycleMutationOutcomeDto::Committed
            } else {
                LifecycleMutationOutcomeDto::RegistryFailed
            },
            record: Some(record),
            path: None,
            recovery: remove_result.err().map(|_| LifecycleRecoveryDto {
                required: true,
                action: "cleanupRegistry".into(),
                path: None,
                identity: None,
            }),
            invalidation: LifecycleInvalidationDto {
                project: false,
                registry: true,
            },
        };
        return Ok(super::lifecycle::publish_lifecycle_result_with(
            result, emit,
        ));
    };
    let prepared = state.prepare_project_deletion(
        std::path::Path::new(&record.path),
        Some(&identity),
        expected_active_instance_id.as_ref(),
        operation_id,
    )?;
    let post_tombstone_failed = prepared.post_tombstone_failed();
    let deleted = state.commit_project_deletion(prepared);
    let registry_removed = if post_tombstone_failed {
        false
    } else {
        catch_future_unwind(remove())
            .await
            .is_ok_and(|result| result.is_ok())
    };
    let result = LifecycleMutationResultDto {
        operation_id,
        kind: LifecycleMutationKindDto::Delete,
        old_project_instance_id: deleted
            .cleared_project_instance_id
            .as_ref()
            .map(ToString::to_string),
        new_project_instance_id: None,
        phase: LifecycleMutationPhaseDto::AuthorityCommitted,
        outcome: if registry_removed {
            LifecycleMutationOutcomeDto::CleanupPending
        } else {
            LifecycleMutationOutcomeDto::RegistryPending
        },
        record: Some(record),
        path: Some(
            deleted
                .deleted_root
                .as_path()
                .to_string_lossy()
                .into_owned(),
        ),
        recovery: Some(LifecycleRecoveryDto {
            required: true,
            action: if registry_removed {
                "cleanupTombstone".into()
            } else {
                "removeRegistryRecord".into()
            },
            path: Some(deleted.tombstone_path.to_string_lossy().into_owned()),
            identity: Some(deleted.tombstone_identity.as_str().to_owned()),
        }),
        invalidation: LifecycleInvalidationDto {
            project: deleted.cleared_project_instance_id.is_some(),
            registry: true,
        },
    };
    Ok(super::lifecycle::publish_lifecycle_result_with(
        result, emit,
    ))
}

#[tauri::command]
pub async fn delete_registered_project_files(
    app: tauri::AppHandle,
    state: State<'_, crate::project::ProjectState>,
    registry: State<'_, ProjectRegistry>,
    id: String,
    expected_active_instance_id: Option<ProjectInstanceId>,
    operation_id: OperationId,
) -> Result<LifecycleMutationResultDto, CommandError> {
    let record = registry
        .fetch_by_id(&id)
        .await
        .map_err(CommandError::internal)?
        .ok_or_else(|| CommandError::expected("project_not_found"))?;

    delete_registered_project_workflow(
        state.inner(),
        record,
        expected_active_instance_id,
        operation_id,
        || async move { registry.remove_project(&id).await },
        |event| crate::event::emit_project_event_result(&app, event),
    )
    .await
}

#[tauri::command]
pub async fn toggle_registered_project_favorite(
    registry: State<'_, ProjectRegistry>,
    id: String,
) -> Result<bool, CommandError> {
    registry
        .toggle_favorite(&id)
        .await
        .map_err(CommandError::internal)
}

#[tauri::command]
pub fn get_project_registry_path(registry: State<ProjectRegistry>) -> String {
    registry.path().to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, EventProject};
    use crate::project::{
        NormalizedProjectRoot, ProjectData, ProjectRootBinding, ProjectRootIdentityState,
        ProjectState, fixtures,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn first_tombstone_bind_replacement_returns_registry_pending_receipt() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir()
                .join(format!("yssbi-delete-workflow-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&root).unwrap();
            fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref()).unwrap();
            let state = ProjectState::new();
            state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
            let session = state.capture_project_session().unwrap();
            let identity = ProjectRootBinding::for_existing(&root)
                .unwrap()
                .identity()
                .unwrap()
                .clone();
            let operation_id = OperationId::new();
            let name = root.file_name().unwrap().to_string_lossy();
            let tombstone = root
                .parent()
                .unwrap()
                .join(format!(".{name}.yssbi-deleting-{operation_id}"));
            let recovery = tombstone.with_extension("external-recovery");
            let record = ProjectRecord {
                id: "registered".into(),
                name: "Registered".into(),
                path: root.to_string_lossy().into_owned(),
                created_at: "2026-01-01T00:00:00Z".into(),
                last_opened_at: None,
                is_favorite: false,
                root_identity: identity.clone(),
                root_identity_state: ProjectRootIdentityState::Valid,
            };
            let emitted = Arc::new(Mutex::new(Vec::new()));
            let events = Arc::clone(&emitted);
            let tombstone_for_hook = tombstone.clone();
            let recovery_for_hook = recovery.clone();
            crate::project::set_after_tombstone_rename_hook(Some(Arc::new(move || {
                std::fs::rename(&tombstone_for_hook, &recovery_for_hook).unwrap();
                std::fs::create_dir_all(&tombstone_for_hook).unwrap();
                std::fs::write(tombstone_for_hook.join("replacement.txt"), b"unrelated").unwrap();
            })));

            let result = delete_registered_project_workflow(
                &state,
                record,
                Some(session.instance_id.clone()),
                operation_id,
                || async { Err("injected registry failure".into()) },
                move |event| {
                    events.lock().unwrap().push(event.clone());
                    Ok(())
                },
            )
            .await
            .unwrap();
            crate::project::set_after_tombstone_rename_hook(None);

            assert_eq!(result.outcome, LifecycleMutationOutcomeDto::RegistryPending);
            assert_eq!(result.operation_id, operation_id);
            let recovery_receipt = result.recovery.as_ref().unwrap();
            assert_eq!(recovery_receipt.action, "removeRegistryRecord");
            let receipt_path = std::path::Path::new(recovery_receipt.path.as_deref().unwrap());
            assert_eq!(receipt_path.file_name(), tombstone.file_name());
            assert!(receipt_path.join("replacement.txt").is_file());
            assert_eq!(
                recovery_receipt.identity.as_deref(),
                Some(identity.as_str())
            );
            assert_eq!(
                std::fs::read(tombstone.join("replacement.txt")).unwrap(),
                b"unrelated"
            );
            assert!(
                recovery
                    .join(crate::project::PROJECT_METADATA_FILE)
                    .is_file()
            );
            assert!(state.capture_project_session().is_err());
            let normalized = NormalizedProjectRoot::from_project_path(&root).unwrap();
            assert_eq!(
                state.filesystem().lifecycle_state_for_test(&normalized),
                (false, false, 0)
            );
            state.clear_project().unwrap();
            assert!(matches!(
                emitted.lock().unwrap().as_slice(),
                [Event::Project(EventProject::ProjectLifecycleCommitted { result: event_result })]
                    if event_result == &result
            ));
            let _ = std::fs::remove_dir_all(tombstone);
            let _ = std::fs::remove_dir_all(recovery);
        });
    }

    #[test]
    fn registry_success_never_recycles_external_tombstone_replacement() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir().join(format!(
                "yssbi-delete-commit-replacement-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref()).unwrap();
            let state = ProjectState::new();
            state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
            let session = state.capture_project_session().unwrap();
            let identity = ProjectRootBinding::for_existing(&root)
                .unwrap()
                .identity()
                .unwrap()
                .clone();
            let operation_id = OperationId::new();
            let name = root.file_name().unwrap().to_string_lossy();
            let tombstone = root
                .parent()
                .unwrap()
                .join(format!(".{name}.yssbi-deleting-{operation_id}"));
            let recovery = tombstone.with_extension("external-recovery");
            let record = ProjectRecord {
                id: "registered".into(),
                name: "Registered".into(),
                path: root.to_string_lossy().into_owned(),
                created_at: "2026-01-01T00:00:00Z".into(),
                last_opened_at: None,
                is_favorite: false,
                root_identity: identity.clone(),
                root_identity_state: ProjectRootIdentityState::Valid,
            };

            let result = delete_registered_project_workflow(
                &state,
                record,
                Some(session.instance_id),
                operation_id,
                || async {
                    std::fs::rename(&tombstone, &recovery).unwrap();
                    std::fs::create_dir_all(&tombstone).unwrap();
                    std::fs::write(tombstone.join("replacement.txt"), b"unrelated").unwrap();
                    Ok(())
                },
                |_| Ok(()),
            )
            .await
            .unwrap();

            assert_eq!(result.outcome, LifecycleMutationOutcomeDto::CleanupPending);
            assert!(state.capture_project_session().is_err());
            assert_eq!(
                std::fs::read(tombstone.join("replacement.txt")).unwrap(),
                b"unrelated"
            );
            assert!(
                recovery
                    .join(crate::project::PROJECT_METADATA_FILE)
                    .is_file()
            );
            assert_eq!(result.recovery.as_ref().unwrap().action, "cleanupTombstone");
            assert_eq!(
                result.recovery.as_ref().unwrap().identity.as_deref(),
                Some(identity.as_str())
            );
            let normalized = NormalizedProjectRoot::from_project_path(&root).unwrap();
            assert_eq!(
                state.filesystem().lifecycle_state_for_test(&normalized),
                (false, false, 0)
            );
            state.clear_project().unwrap();
            let _ = std::fs::remove_dir_all(tombstone);
            let _ = std::fs::remove_dir_all(recovery);
        });
    }

    #[test]
    fn registry_future_panic_returns_exact_pending_receipt_and_releases_ownership() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir().join(format!(
                "yssbi-delete-registry-panic-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref()).unwrap();
            let state = ProjectState::new();
            state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
            let session = state.capture_project_session().unwrap();
            let binding = ProjectRootBinding::for_existing(&root).unwrap();
            let normalized = binding.normalized().clone();
            let committed_root = normalized.as_path().to_path_buf();
            let identity = binding.identity().unwrap().clone();
            let operation_id = OperationId::new();
            let tombstone = committed_root.parent().unwrap().join(format!(
                ".{}.yssbi-deleting-{operation_id}",
                committed_root.file_name().unwrap().to_string_lossy()
            ));
            let record = ProjectRecord {
                id: "registered".into(),
                name: "Registered".into(),
                path: root.to_string_lossy().into_owned(),
                created_at: "2026-01-01T00:00:00Z".into(),
                last_opened_at: None,
                is_favorite: false,
                root_identity: identity.clone(),
                root_identity_state: ProjectRootIdentityState::Valid,
            };
            let expected = LifecycleMutationResultDto {
                operation_id,
                kind: LifecycleMutationKindDto::Delete,
                old_project_instance_id: Some(session.instance_id.to_string()),
                new_project_instance_id: None,
                phase: LifecycleMutationPhaseDto::AuthorityCommitted,
                outcome: LifecycleMutationOutcomeDto::RegistryPending,
                record: Some(record.clone()),
                path: Some(committed_root.to_string_lossy().into_owned()),
                recovery: Some(LifecycleRecoveryDto {
                    required: true,
                    action: "removeRegistryRecord".into(),
                    path: Some(tombstone.to_string_lossy().into_owned()),
                    identity: Some(identity.as_str().to_owned()),
                }),
                invalidation: LifecycleInvalidationDto {
                    project: true,
                    registry: true,
                },
            };
            let emitted = Arc::new(Mutex::new(Vec::new()));
            let events = Arc::clone(&emitted);
            let filesystem = state.filesystem().clone();
            let root_during_registry = normalized.clone();

            let result = delete_registered_project_workflow(
                &state,
                record,
                Some(session.instance_id),
                operation_id,
                || async move {
                    assert_eq!(
                        filesystem.lifecycle_state_for_test(&root_during_registry),
                        (false, false, 0)
                    );
                    panic!("injected registry future panic")
                },
                move |event| {
                    events.lock().unwrap().push(event.clone());
                    Ok(())
                },
            )
            .await
            .unwrap();

            assert_eq!(result, expected);
            assert!(matches!(
                emitted.lock().unwrap().as_slice(),
                [Event::Project(EventProject::ProjectLifecycleCommitted { result: event_result })]
                    if event_result == &expected
            ));
            assert!(state.capture_project_session().is_err());
            assert_eq!(
                state.filesystem().lifecycle_state_for_test(&normalized),
                (false, false, 0)
            );
            drop(state.filesystem().acquire(normalized).unwrap());
            let _ = std::fs::remove_dir_all(tombstone);
        });
    }

    #[test]
    fn post_rename_hook_panic_returns_exact_pending_receipt_and_releases_ownership() {
        struct ResetHook;
        impl Drop for ResetHook {
            fn drop(&mut self) {
                crate::project::set_after_tombstone_rename_hook(None);
            }
        }

        tauri::async_runtime::block_on(async {
            let _reset = ResetHook;
            let root = std::env::temp_dir().join(format!(
                "yssbi-delete-post-rename-panic-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref()).unwrap();
            let state = ProjectState::new();
            state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
            let session = state.capture_project_session().unwrap();
            let binding = ProjectRootBinding::for_existing(&root).unwrap();
            let normalized = binding.normalized().clone();
            let committed_root = normalized.as_path().to_path_buf();
            let identity = binding.identity().unwrap().clone();
            let operation_id = OperationId::new();
            let tombstone = committed_root.parent().unwrap().join(format!(
                ".{}.yssbi-deleting-{operation_id}",
                committed_root.file_name().unwrap().to_string_lossy()
            ));
            let record = ProjectRecord {
                id: "registered".into(),
                name: "Registered".into(),
                path: root.to_string_lossy().into_owned(),
                created_at: "2026-01-01T00:00:00Z".into(),
                last_opened_at: None,
                is_favorite: false,
                root_identity: identity.clone(),
                root_identity_state: ProjectRootIdentityState::Valid,
            };
            let expected = LifecycleMutationResultDto {
                operation_id,
                kind: LifecycleMutationKindDto::Delete,
                old_project_instance_id: Some(session.instance_id.to_string()),
                new_project_instance_id: None,
                phase: LifecycleMutationPhaseDto::AuthorityCommitted,
                outcome: LifecycleMutationOutcomeDto::RegistryPending,
                record: Some(record.clone()),
                path: Some(committed_root.to_string_lossy().into_owned()),
                recovery: Some(LifecycleRecoveryDto {
                    required: true,
                    action: "removeRegistryRecord".into(),
                    path: Some(tombstone.to_string_lossy().into_owned()),
                    identity: Some(identity.as_str().to_owned()),
                }),
                invalidation: LifecycleInvalidationDto {
                    project: true,
                    registry: true,
                },
            };
            let emitted = Arc::new(Mutex::new(Vec::new()));
            let events = Arc::clone(&emitted);
            crate::project::set_after_tombstone_rename_hook(Some(Arc::new(|| {
                panic!("injected post-rename panic")
            })));

            let result = delete_registered_project_workflow(
                &state,
                record,
                Some(session.instance_id),
                operation_id,
                || async { Ok(()) },
                move |event| {
                    events.lock().unwrap().push(event.clone());
                    Ok(())
                },
            )
            .await
            .unwrap();

            assert_eq!(result, expected);
            assert!(matches!(
                emitted.lock().unwrap().as_slice(),
                [Event::Project(EventProject::ProjectLifecycleCommitted { result: event_result })]
                    if event_result == &expected
            ));
            assert!(state.capture_project_session().is_err());
            assert_eq!(
                state.filesystem().lifecycle_state_for_test(&normalized),
                (false, false, 0)
            );
            drop(state.filesystem().acquire(normalized).unwrap());
            let _ = std::fs::remove_dir_all(tombstone);
        });
    }

    #[test]
    fn invalid_active_row_is_rejected_as_registry_cleanup_without_file_deletion() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir().join(format!(
                "yssbi-invalid-active-registry-cleanup-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("sentinel.txt"), b"preserve").unwrap();
            let state = ProjectState::new();
            state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
            let session = state.capture_project_session().unwrap();
            let record = ProjectRecord {
                id: "invalid".into(),
                name: "Invalid".into(),
                path: root.to_string_lossy().into_owned(),
                created_at: "2026-01-01T00:00:00Z".into(),
                last_opened_at: None,
                is_favorite: false,
                root_identity: crate::project::ProjectRootIdentity::from_stored(String::new()),
                root_identity_state: ProjectRootIdentityState::Invalid,
            };
            let removed = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let remove_called = Arc::clone(&removed);
            let operation_id = OperationId::new();
            let expected = LifecycleMutationResultDto {
                operation_id,
                kind: LifecycleMutationKindDto::RegistryCleanup,
                old_project_instance_id: None,
                new_project_instance_id: None,
                phase: LifecycleMutationPhaseDto::RegistryCommitted,
                outcome: LifecycleMutationOutcomeDto::RegistryFailed,
                record: Some(record.clone()),
                path: None,
                recovery: Some(LifecycleRecoveryDto {
                    required: true,
                    action: "cleanupRegistry".into(),
                    path: None,
                    identity: None,
                }),
                invalidation: LifecycleInvalidationDto {
                    project: false,
                    registry: true,
                },
            };
            let emitted = Arc::new(Mutex::new(Vec::new()));
            let events = Arc::clone(&emitted);

            let result = delete_registered_project_workflow(
                &state,
                record,
                Some(session.instance_id.clone()),
                operation_id,
                move || async move {
                    remove_called.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
                move |event| {
                    events.lock().unwrap().push(event.clone());
                    Ok(())
                },
            )
            .await
            .unwrap();

            assert!(!removed.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!(result, expected);
            assert!(matches!(
                emitted.lock().unwrap().as_slice(),
                [Event::Project(EventProject::ProjectLifecycleCommitted { result: event_result })]
                    if event_result == &expected
            ));
            assert_eq!(state.capture_project_session().unwrap(), session);
            assert_eq!(
                std::fs::read(root.join("sentinel.txt")).unwrap(),
                b"preserve"
            );
            let _ = std::fs::remove_dir_all(root);
        });
    }

    #[test]
    fn registry_failure_commits_clear_and_returns_registry_pending_with_released_ownership() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir().join(format!(
                "yssbi-delete-registry-rollback-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref()).unwrap();
            let state = ProjectState::new();
            state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
            let session = state.capture_project_session().unwrap();
            let binding = ProjectRootBinding::for_existing(&root).unwrap();
            let normalized = binding.normalized().clone();
            let registry_dir = std::env::temp_dir()
                .join(format!("yssbi-delete-registry-db-{}", uuid::Uuid::new_v4()));
            let registry = ProjectRegistry::init(registry_dir.clone()).await.unwrap();
            let record = registry
                .register_project("Registered", root.to_string_lossy().as_ref())
                .await
                .unwrap();
            let record_id = record.id.clone();
            registry.fail_project_remove_for_test().await;

            let result = delete_registered_project_workflow(
                &state,
                record,
                Some(session.instance_id.clone()),
                OperationId::new(),
                || registry.remove_project(&record_id),
                |_| Ok(()),
            )
            .await
            .unwrap();

            assert_eq!(result.outcome, LifecycleMutationOutcomeDto::RegistryPending);
            assert_eq!(
                result.recovery.as_ref().unwrap().action,
                "removeRegistryRecord"
            );
            assert!(!root.exists());
            assert!(state.capture_project_session().is_err());
            assert_eq!(
                state.filesystem().lifecycle_state_for_test(&normalized),
                (false, false, 0)
            );
            let lease = state.filesystem().acquire(normalized).unwrap();
            drop(lease);
            state.clear_project().unwrap();
            drop(registry);
            let _ = std::fs::remove_dir_all(root);
            let _ = std::fs::remove_dir_all(registry_dir);
        });
    }
}
