use crate::error::CommandError;
use crate::event::{
    Event, EventProject, LifecycleInvalidationDto, LifecycleMutationKindDto,
    LifecycleMutationOutcomeDto, LifecycleMutationPhaseDto, LifecycleMutationResultDto,
    LifecycleRecoveryDto, ProjectActivationResultDto, emit_project_event,
    emit_project_event_result,
};

use crate::node_system::document::OperationId;
use crate::project::project_writers::ProjectSaveResultDto;
use crate::project::{
    PreparedProjectActivation, ProjectFilesystemError, ProjectInstanceId, ProjectRecord,
    ProjectRegistry, ProjectSession, ProjectState, ProjectWatcherState, normalize_existing_path,
};
use std::future::Future;
use std::path::Path;
use tauri::{AppHandle, State};

fn emit_project_loaded(app: &AppHandle, result: ProjectActivationResultDto) {
    emit_project_event(app, Event::Project(EventProject::ProjectLoaded { result }));
}

fn start_project_watcher(
    app: &AppHandle,
    watcher: &ProjectWatcherState,
    path: &str,
    project_instance_id: &ProjectInstanceId,
) {
    if let Err(error) = watcher.watch_project(app.clone(), path, project_instance_id.clone()) {
        tracing::warn!(
            target: "yssbi::project::watcher",
            diagnostic_domain = "system",
            error = %error,
            "Failed to start project watcher"
        );
    }
}

/// 加载项目（从状态管理层）
#[tauri::command]
pub fn load_project(
    app: AppHandle,
    state: State<ProjectState>,
    watcher: State<ProjectWatcherState>,
    path: String,
) -> Result<ProjectActivationResultDto, CommandError> {
    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "loadProject",
        path = path.as_str(),
        "Loading project"
    );

    let path =
        normalize_existing_path(&path).map_err(|_| CommandError::expected("invalid_path"))?;

    let session = state
        .activate_project_from_path(std::path::Path::new(&path))
        .map_err(|error| CommandError::diagnosed("load_project_failed", error))?;
    let project_data = state
        .get_data()
        .map_err(|error| CommandError::diagnosed("load_project_failed", error))?;

    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "projectLoaded",
        project = %project_data.info(),
        "Project loaded"
    );

    start_project_watcher(&app, &watcher, &path, &session.instance_id);
    let result = ProjectActivationResultDto {
        path,
        project_instance_id: session.instance_id.to_string(),
        activation_revision: state.activation_revision(),
    };
    emit_project_loaded(&app, result.clone());
    Ok(result)
}

async fn save_project_as_workflow<Register, RegisterFuture, Activate, Emit>(
    state: &ProjectState,
    destination: &Path,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    register: Register,
    activate: Activate,
    emit: Emit,
) -> Result<LifecycleMutationResultDto, CommandError>
where
    Register: FnOnce(String, String) -> RegisterFuture,
    RegisterFuture: Future<Output = Result<ProjectRecord, String>>,
    Activate: FnOnce(PreparedProjectActivation) -> Result<ProjectSession, ProjectFilesystemError>,
    Emit: FnOnce(&Event) -> Result<(), String>,
{
    let prepared =
        state.save_project_as_transaction(&project_instance_id, destination, operation_id)?;
    let metadata_path = prepared.metadata_path.to_string_lossy().into_owned();
    let project_name = prepared
        .prepared_activation
        .data
        .metadata
        .project_name
        .clone();
    let record = match register(project_name, metadata_path.clone()).await {
        Ok(record) => record,
        Err(_) => {
            return Ok(publish_lifecycle_result_with(
                lifecycle_failure_result(
                    operation_id,
                    LifecycleMutationKindDto::SaveAs,
                    Some(project_instance_id.to_string()),
                    LifecycleMutationPhaseDto::DestinationCommitted,
                    LifecycleMutationOutcomeDto::RegistryFailed,
                    None,
                    metadata_path,
                    "registerDestination",
                    false,
                ),
                emit,
            ));
        }
    };
    let session = match activate(prepared.prepared_activation) {
        Ok(session) => session,
        Err(_) => {
            return Ok(publish_lifecycle_result_with(
                lifecycle_failure_result(
                    operation_id,
                    LifecycleMutationKindDto::SaveAs,
                    Some(project_instance_id.to_string()),
                    LifecycleMutationPhaseDto::RegistryCommitted,
                    LifecycleMutationOutcomeDto::ActivationFailed,
                    Some(record),
                    metadata_path,
                    "activateDestination",
                    true,
                ),
                emit,
            ));
        }
    };
    Ok(publish_lifecycle_result_with(
        LifecycleMutationResultDto {
            operation_id,
            kind: LifecycleMutationKindDto::SaveAs,
            old_project_instance_id: Some(project_instance_id.to_string()),
            new_project_instance_id: Some(session.instance_id.to_string()),
            phase: LifecycleMutationPhaseDto::AuthorityCommitted,
            outcome: LifecycleMutationOutcomeDto::Committed,
            record: Some(record),
            path: Some(metadata_path),
            recovery: None,
            invalidation: LifecycleInvalidationDto {
                project: true,
                registry: true,
            },
        },
        emit,
    ))
}

/// 将当前项目另存为新目录（完整复制 events/functions/database 等）。
#[tauri::command]
pub async fn save_project_as(
    app: AppHandle,
    state: State<'_, ProjectState>,
    watcher: State<'_, ProjectWatcherState>,
    registry: State<'_, ProjectRegistry>,
    path: String,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
) -> Result<LifecycleMutationResultDto, CommandError> {
    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "saveProjectAs",
        path = path.as_str(),
        "Saving project copy"
    );

    let result = save_project_as_workflow(
        state.inner(),
        Path::new(&path),
        project_instance_id,
        operation_id,
        |name, metadata_path| async move { registry.register_project(&name, &metadata_path).await },
        |prepared| state.activate_prepared_project(prepared),
        |event| emit_project_event_result(&app, event),
    )
    .await?;
    if result.outcome == LifecycleMutationOutcomeDto::Committed {
        if let (Some(metadata_path), Some(project_instance_id)) = (
            result.path.as_deref(),
            result.new_project_instance_id.as_deref(),
        ) {
            let project_instance_id = ProjectInstanceId::from_existing(project_instance_id.into());
            start_project_watcher(&app, &watcher, metadata_path, &project_instance_id);
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn create_project(
    app: AppHandle,
    state: State<'_, ProjectState>,
    registry: State<'_, ProjectRegistry>,
    name: String,
    path: String,
    operation_id: OperationId,
) -> Result<LifecycleMutationResultDto, CommandError> {
    let created =
        state.create_project_transaction(&name, std::path::Path::new(&path), operation_id)?;
    let metadata_path = created.metadata_path.to_string_lossy().into_owned();
    let result = match registry
        .register_project(&created.project_name, &metadata_path)
        .await
    {
        Ok(record) => LifecycleMutationResultDto {
            operation_id,
            kind: LifecycleMutationKindDto::Create,
            old_project_instance_id: None,
            new_project_instance_id: None,
            phase: LifecycleMutationPhaseDto::RegistryCommitted,
            outcome: LifecycleMutationOutcomeDto::Committed,
            record: Some(record),
            path: Some(metadata_path),
            recovery: None,
            invalidation: LifecycleInvalidationDto {
                project: false,
                registry: true,
            },
        },
        Err(_) => lifecycle_failure_result(
            operation_id,
            LifecycleMutationKindDto::Create,
            None,
            LifecycleMutationPhaseDto::DestinationCommitted,
            LifecycleMutationOutcomeDto::RegistryFailed,
            None,
            metadata_path,
            "registerDestination",
            false,
        ),
    };
    Ok(publish_lifecycle_result(&app, result))
}

fn lifecycle_failure_result(
    operation_id: OperationId,
    kind: LifecycleMutationKindDto,
    old_project_instance_id: Option<String>,
    phase: LifecycleMutationPhaseDto,
    outcome: LifecycleMutationOutcomeDto,
    record: Option<ProjectRecord>,
    path: String,
    action: &str,
    project_invalidation: bool,
) -> LifecycleMutationResultDto {
    LifecycleMutationResultDto {
        operation_id,
        kind,
        old_project_instance_id,
        new_project_instance_id: None,
        phase,
        outcome,
        record,
        path: Some(path.clone()),
        recovery: Some(LifecycleRecoveryDto {
            required: true,
            action: action.into(),
            path: Some(path),
            identity: None,
        }),
        invalidation: LifecycleInvalidationDto {
            project: project_invalidation,
            registry: true,
        },
    }
}

pub(crate) fn publish_lifecycle_result_with(
    result: LifecycleMutationResultDto,
    emit: impl FnOnce(&Event) -> Result<(), String>,
) -> LifecycleMutationResultDto {
    let event = Event::Project(EventProject::ProjectLifecycleCommitted {
        result: result.clone(),
    });
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| emit(&event)));
    result
}

pub(crate) fn publish_lifecycle_result(
    app: &AppHandle,
    result: LifecycleMutationResultDto,
) -> LifecycleMutationResultDto {
    publish_lifecycle_result_with(result, |event| {
        emit_project_event_result(app, event).inspect_err(|error| {
            tracing::error!(
                target: "yssbi::project::events",
                diagnostic_domain = "application",
                diagnostic_event = "lifecycleEventEmitFailed",
                error = %error,
                "Failed to emit project lifecycle event"
            );
        })
    })
}

pub(crate) fn flush_project_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    mut emit: impl FnMut(Event),
) -> Result<ProjectSaveResultDto, CommandError> {
    let result = state
        .flush_project_documents(&project_instance_id, operation_id)
        .map_err(CommandError::from)?;
    emit(Event::Project(EventProject::ProjectSaved {
        result: result.clone(),
    }));
    Ok(result)
}

#[tauri::command]
pub fn flush_project(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
) -> Result<ProjectSaveResultDto, CommandError> {
    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "flushProject",
        "Flushing project"
    );
    flush_project_with_emitter(state.inner(), project_instance_id, operation_id, |event| {
        emit_project_event(&app, event)
    })
}

/// 新建项目（清空当前状态）
#[tauri::command]
pub fn new_project(
    app: AppHandle,
    state: State<ProjectState>,
    watcher: State<ProjectWatcherState>,
) -> Result<(), CommandError> {
    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "newProject",
        "Creating new project"
    );

    state.clear_project()?;
    watcher.stop();
    emit_project_event(&app, Event::Project(EventProject::ProjectCleared));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::document::OperationId;
    use crate::project::ProjectData;

    #[test]
    fn save_as_registry_failure_preserves_source_and_disk_with_exact_receipt() {
        tauri::async_runtime::block_on(async {
            let source = std::env::temp_dir().join(format!(
                "yssbi-save-as-workflow-source-{}",
                uuid::Uuid::new_v4()
            ));
            let destination = std::env::temp_dir().join(format!(
                "yssbi-save-as-workflow-destination-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&source).unwrap();
            let mut data = ProjectData::new();
            data.metadata.project_name = "Workflow".into();
            crate::project::fixtures::write_project(&data, source.to_string_lossy().as_ref())
                .unwrap();
            let state = ProjectState::new();
            state.activate_project_fixture(source.to_string_lossy().into_owned(), data);
            let source_session = state.capture_project_session().unwrap();
            let operation_id = OperationId::new();
            let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let emitted = std::sync::Arc::clone(&events);

            let result = save_project_as_workflow(
                &state,
                &destination,
                source_session.instance_id.clone(),
                operation_id,
                |_name, _metadata_path| async { Err("injected registry failure".into()) },
                |prepared| state.activate_prepared_project(prepared),
                move |event| {
                    emitted.lock().unwrap().push(event.clone());
                    Ok(())
                },
            )
            .await
            .unwrap();

            assert_eq!(result.operation_id, operation_id);
            assert_eq!(result.outcome, LifecycleMutationOutcomeDto::RegistryFailed);
            assert!(
                destination
                    .join(crate::project::PROJECT_METADATA_FILE)
                    .is_file()
            );
            assert_eq!(state.capture_project_session().unwrap(), source_session);
            assert!(source.join(crate::project::PROJECT_METADATA_FILE).is_file());
            assert!(matches!(
                events.lock().unwrap().as_slice(),
                [Event::Project(EventProject::ProjectLifecycleCommitted { result: emitted })]
                    if emitted == &result
            ));
            let _ = std::fs::remove_dir_all(source);
            let _ = std::fs::remove_dir_all(destination);
        });
    }

    #[test]
    fn save_as_activation_failure_and_event_failure_return_exact_direct_receipts() {
        tauri::async_runtime::block_on(async {
            for fail_activation in [true, false] {
                let source = std::env::temp_dir().join(format!(
                    "yssbi-save-as-boundary-source-{}",
                    uuid::Uuid::new_v4()
                ));
                let destination = std::env::temp_dir().join(format!(
                    "yssbi-save-as-boundary-destination-{}",
                    uuid::Uuid::new_v4()
                ));
                std::fs::create_dir_all(&source).unwrap();
                let mut data = ProjectData::new();
                data.metadata.project_name = "Boundary".into();
                crate::project::fixtures::write_project(&data, source.to_string_lossy().as_ref())
                    .unwrap();
                let state = ProjectState::new();
                state.activate_project_fixture(source.to_string_lossy().into_owned(), data);
                let source_session = state.capture_project_session().unwrap();
                let identity = crate::project::ProjectRootBinding::for_existing(&destination).err();
                assert!(identity.is_some());
                let emitted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let emit_count = std::sync::Arc::clone(&emitted);
                let register_path = destination
                    .join(crate::project::PROJECT_METADATA_FILE)
                    .to_string_lossy()
                    .into_owned();

                let result = save_project_as_workflow(
                    &state,
                    &destination,
                    source_session.instance_id.clone(),
                    OperationId::new(),
                    move |_name, _metadata_path| {
                        let register_path = register_path.clone();
                        async move {
                            let binding =
                                crate::project::ProjectRootBinding::for_existing(&register_path)
                                    .unwrap();
                            Ok(ProjectRecord {
                                id: "registered".into(),
                                name: "Boundary".into(),
                                path: register_path,
                                created_at: "2026-01-01T00:00:00Z".into(),
                                last_opened_at: None,
                                is_favorite: false,
                                root_identity: binding.identity().unwrap().clone(),
                                root_identity_state:
                                    crate::project::ProjectRootIdentityState::Valid,
                            })
                        }
                    },
                    |prepared| {
                        if fail_activation {
                            Err(ProjectFilesystemError::StaleProjectLifecycle {
                                message: "injected activation failure".into(),
                            })
                        } else {
                            state.activate_prepared_project(prepared)
                        }
                    },
                    move |_event| {
                        emit_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if fail_activation {
                            Ok(())
                        } else {
                            Err("event transport unavailable".into())
                        }
                    },
                )
                .await
                .unwrap();

                assert_eq!(emitted.load(std::sync::atomic::Ordering::SeqCst), 1);
                assert!(
                    destination
                        .join(crate::project::PROJECT_METADATA_FILE)
                        .is_file()
                );
                assert_eq!(result.record.as_ref().unwrap().id, "registered");
                if fail_activation {
                    assert_eq!(
                        result.outcome,
                        LifecycleMutationOutcomeDto::ActivationFailed
                    );
                    assert_eq!(state.capture_project_session().unwrap(), source_session);
                } else {
                    assert_eq!(result.outcome, LifecycleMutationOutcomeDto::Committed);
                    assert_ne!(state.capture_project_session().unwrap(), source_session);
                }
                let _ = std::fs::remove_dir_all(source);
                let _ = std::fs::remove_dir_all(destination);
            }
        });
    }

    #[test]
    fn registry_failure_reports_preserved_created_project() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-create-registry-failure-{}",
            uuid::Uuid::new_v4()
        ));
        let state = ProjectState::new();
        let created = state
            .create_project_transaction("Preserved", &root, OperationId::new())
            .unwrap();
        let metadata_path = created.metadata_path.to_string_lossy().into_owned();

        let operation_id = OperationId::new();
        let result = lifecycle_failure_result(
            operation_id,
            LifecycleMutationKindDto::Create,
            None,
            LifecycleMutationPhaseDto::DestinationCommitted,
            LifecycleMutationOutcomeDto::RegistryFailed,
            None,
            metadata_path.clone(),
            "registerDestination",
            false,
        );

        assert_eq!(result.operation_id, operation_id);
        assert_eq!(result.outcome, LifecycleMutationOutcomeDto::RegistryFailed);
        assert_eq!(result.path.as_deref(), Some(metadata_path.as_str()));
        assert_eq!(
            result.recovery.as_ref().unwrap().action,
            "registerDestination"
        );
        assert!(created.metadata_path.is_file());
        assert!(root.join(crate::project::DATABASE_DIR).is_dir());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lifecycle_event_panic_preserves_direct_committed_receipt() {
        let result = lifecycle_failure_result(
            OperationId::new(),
            LifecycleMutationKindDto::Delete,
            Some("old-project".into()),
            LifecycleMutationPhaseDto::AuthorityCommitted,
            LifecycleMutationOutcomeDto::RegistryPending,
            None,
            "C:/project".into(),
            "removeRegistryRecord",
            true,
        );

        let direct = publish_lifecycle_result_with(result.clone(), |_| {
            panic!("injected lifecycle emitter panic")
        });

        assert_eq!(direct, result);
    }

    #[test]
    fn lifecycle_event_failure_preserves_direct_committed_receipt() {
        let result = lifecycle_failure_result(
            OperationId::new(),
            LifecycleMutationKindDto::Delete,
            Some("old-project".into()),
            LifecycleMutationPhaseDto::AuthorityCommitted,
            LifecycleMutationOutcomeDto::CleanupPending,
            None,
            "C:/project".into(),
            "cleanupTombstone",
            true,
        );

        let direct = publish_lifecycle_result_with(result.clone(), |_| {
            Err("event transport unavailable".to_string())
        });

        assert_eq!(direct, result);
    }

    #[test]
    fn flush_command_returns_correlated_result_emits_once_and_stale_emits_nothing() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-flush-command-contract-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let operation_id = OperationId::new();
        let mut events = Vec::new();

        let result = flush_project_with_emitter(
            &state,
            project_instance_id.clone(),
            operation_id,
            |event| events.push(event),
        )
        .unwrap();

        assert_eq!(result.project_instance_id, project_instance_id.as_str());
        assert_eq!(result.operation_id, operation_id);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            Event::Project(EventProject::ProjectSaved { result: emitted }) if emitted == &result
        ));

        state.activate_project_fixture(
            root.to_string_lossy().into_owned(),
            state.get_data().unwrap(),
        );
        let error =
            flush_project_with_emitter(&state, project_instance_id, OperationId::new(), |event| {
                events.push(event)
            })
            .unwrap_err();
        assert_eq!(error.code(), "stale_project_lifecycle");
        assert_eq!(events.len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }
}
