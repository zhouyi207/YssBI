use crate::application::execution::{ApplicationState, SessionCaptureError};
use crate::application::project_change::ApplicationProjectWatchError;
use crate::application::project_lifecycle::ProjectLifecycleError;
use crate::error::CommandError;
use crate::event::{
    Event, EventProject, EventResource, emit_project_event, emit_project_event_result,
};
#[cfg(test)]
use crate::project::ProjectState;
use crate::schema::ProjectSaveResultDto;
use crate::schema::application_event::{
    LifecycleMutationOutcomeDto, LifecycleMutationResultDto, ProjectActivationResultDto,
};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use tauri::{AppHandle, State};
use yss_project_identity::OperationId;
use yss_project_identity::ProjectInstanceId;
use yss_project_registry::ProjectRegistry;
use yss_project_watcher::{
    ObservedProjectChange, ProjectChangeSink, ProjectWatcherError, ProjectWatcherState,
};

fn emit_project_loaded(app: &AppHandle, result: ProjectActivationResultDto) {
    emit_project_event(app, Event::Project(EventProject::ProjectLoaded { result }));
}

pub(crate) fn map_project_lifecycle_error(error: ProjectLifecycleError) -> CommandError {
    match error {
        ProjectLifecycleError::InvalidPath => CommandError::expected("invalid_path"),
        ProjectLifecycleError::ProjectNotFound => CommandError::expected("project_not_found"),
        ProjectLifecycleError::LoadFailed(source) => {
            CommandError::diagnosed("load_project_failed", source)
        }
        ProjectLifecycleError::AuthorityFailed(source) => {
            crate::commands::project_failure::application_project_command_error(source)
        }
        ProjectLifecycleError::RegistryLookupFailed(source) => CommandError::internal(source),
    }
}

pub(super) fn map_application_project_lifecycle_error(
    error: crate::application::project_lifecycle::ApplicationProjectLifecycleError,
) -> CommandError {
    match error {
        crate::application::project_lifecycle::ApplicationProjectLifecycleError::SessionCapture(
            error,
        ) => match error {
            SessionCaptureError::Inactive => CommandError::expected("stale_project_lifecycle"),
            SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            SessionCaptureError::Recovering => CommandError::expected("project_recovery_required")
                .with_details(serde_json::json!({ "recoveryRequired": true })),
        },
        crate::application::project_lifecycle::ApplicationProjectLifecycleError::Lifecycle(
            error,
        ) => map_project_lifecycle_error(error),
        crate::application::project_lifecycle::ApplicationProjectLifecycleError::SessionChanged(
            error,
        ) => CommandError::diagnosed("project_lifecycle_session_changed", error),
        crate::application::project_lifecycle::ApplicationProjectLifecycleError::SessionRefresh(
            error,
        ) => CommandError::diagnosed("project_session_refresh_failed", error),
    }
}

fn start_project_watcher(
    app: &AppHandle,
    application: &ApplicationState,
    watcher: &ProjectWatcherState,
    path: &str,
    project_instance_id: &ProjectInstanceId,
) {
    let sink = Arc::new(ProjectEventWatcherSink {
        app: app.clone(),
        application: application.clone(),
        project_instance_id: project_instance_id.clone(),
        version: Mutex::new(0),
    });
    if let Err(error) = watcher.watch_project(path, sink) {
        tracing::warn!(
            target: "yssbi::project::watcher",
            diagnostic_domain = "system",
            error_kind = watcher_error_kind(&error),
            error = %error,
            "Failed to start project watcher"
        );
    }
}

struct ProjectEventWatcherSink {
    app: AppHandle,
    application: ApplicationState,
    project_instance_id: ProjectInstanceId,
    version: Mutex<u64>,
}

impl ProjectChangeSink for ProjectEventWatcherSink {
    fn publish(&self, change: ObservedProjectChange) {
        let invalidation = match self
            .application
            .reconcile_project_change(&self.project_instance_id, change.change)
        {
            Ok(Some(invalidation)) => invalidation,
            Ok(None) => return,
            Err(ApplicationProjectWatchError::Reconciliation(error)) => {
                tracing::warn!(
                    target: "yssbi::project::watcher",
                    diagnostic_domain = "system",
                    diagnostic_event = "projectIndexRefreshFailed",
                    error_kind = "reconciliation_failed",
                    error = %error,
                    "Failed to reconcile watched project file change"
                );
                return;
            }
            Err(error) => {
                tracing::warn!(
                    target: "yssbi::project::watcher",
                    diagnostic_domain = "system",
                    diagnostic_event = "projectIndexRefreshFailed",
                    error_kind = application_watch_error_kind(&error),
                    "Failed to reconcile watched project file"
                );
                return;
            }
        };

        let project_instance_id = invalidation.into_project_instance_id();
        let Some(version) = next_watcher_version(&self.version) else {
            tracing::error!(
                target: "yssbi::project::watcher",
                diagnostic_domain = "system",
                diagnostic_event = "watcherVersionExhausted",
                "Project watcher event version is exhausted"
            );
            return;
        };
        if let Err(error) = emit_project_event_result(
            &self.app,
            &Event::Resource(EventResource::ProjectIndexInvalidated {
                project_instance_id,
                source: "watcher".to_owned(),
                version,
            }),
        ) {
            tracing::warn!(
                target: "yssbi::project::watcher",
                diagnostic_domain = "system",
                diagnostic_event = "projectEventEmitFailed",
                error = %error,
                "Failed to emit project index invalidation"
            );
        }
    }
}

fn watcher_error_kind(error: &ProjectWatcherError) -> &'static str {
    match error {
        ProjectWatcherError::Start(_) => "source_start_failed",
        ProjectWatcherError::EpochExhausted => "epoch_exhausted",
        ProjectWatcherError::TimedOut(_) => "drain_timeout",
    }
}

fn application_watch_error_kind(error: &ApplicationProjectWatchError) -> &'static str {
    match error {
        ApplicationProjectWatchError::SessionCapture(_) => "session_capture_failed",
        ApplicationProjectWatchError::ProjectIdentityMismatch => "stale_project_lifecycle",
        ApplicationProjectWatchError::Reconciliation(_) => "reconciliation_failed",
        ApplicationProjectWatchError::SessionChanged => "session_changed",
    }
}

fn next_watcher_version(version: &Mutex<u64>) -> Option<u64> {
    let mut version = version.lock().unwrap_or_else(PoisonError::into_inner);
    let next = version.checked_add(1)?;
    *version = next;
    Some(next)
}

/// 加载项目（从状态管理层）
#[tauri::command]
pub fn load_project(
    app: AppHandle,
    application: State<'_, ApplicationState>,
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

    let result = application
        .load_project_for_application(&path)
        .map_err(map_application_project_lifecycle_error)?;

    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "projectLoaded",
        project_instance_id = result.project_instance_id.as_str(),
        "Project loaded"
    );

    let project_instance_id = result.project_instance_id.clone();
    start_project_watcher(
        &app,
        application.inner(),
        &watcher,
        &result.path,
        &project_instance_id,
    );
    let result = crate::schema::application_event::project_activation_to_transport(&result);
    emit_project_loaded(&app, result.clone());
    Ok(result)
}

/// 将当前项目另存为新目录（完整复制 events/functions/database 等）。
#[tauri::command]
pub async fn save_project_as(
    app: AppHandle,
    application: State<'_, ApplicationState>,
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

    let result = application
        .save_project_as_for_application(
            registry.inner(),
            Path::new(&path),
            project_instance_id,
            operation_id,
        )
        .await
        .map_err(map_application_project_lifecycle_error)?;
    let transport = crate::schema::application_event::project_lifecycle_to_transport(&result);
    publish_lifecycle_result(&app, &transport);
    if result.outcome == crate::application::events::ProjectLifecycleOutcome::Committed {
        if let (Some(metadata_path), Some(project_instance_id)) = (
            result.path.as_deref(),
            result.new_project_instance_id.as_ref(),
        ) {
            start_project_watcher(
                &app,
                application.inner(),
                &watcher,
                metadata_path,
                project_instance_id,
            );
        }
    }
    Ok(transport)
}

#[tauri::command]
pub async fn create_project(
    app: AppHandle,
    application: State<'_, ApplicationState>,
    registry: State<'_, ProjectRegistry>,
    name: String,
    path: String,
    operation_id: OperationId,
) -> Result<LifecycleMutationResultDto, CommandError> {
    let result = application
        .create_project_for_application(registry.inner(), &name, Path::new(&path), operation_id)
        .await
        .map_err(map_application_project_lifecycle_error)?;
    let result = crate::schema::application_event::project_lifecycle_to_transport(&result);
    publish_lifecycle_result(&app, &result);
    Ok(result)
}

pub(crate) fn publish_lifecycle_result_with(
    result: &LifecycleMutationResultDto,
    emit: impl FnOnce(&Event) -> Result<(), String>,
) {
    let event = Event::Project(EventProject::ProjectLifecycleCommitted {
        result: result.clone(),
    });
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| emit(&event)));
}

pub(crate) fn publish_lifecycle_result(app: &AppHandle, result: &LifecycleMutationResultDto) {
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

#[cfg(test)]
pub(crate) fn flush_project_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    mut emit: impl FnMut(Event),
) -> Result<ProjectSaveResultDto, CommandError> {
    let result = state
        .flush_project_documents(&project_instance_id, operation_id)
        .map_err(crate::commands::project_failure::application_project_command_error)?;
    let result = ProjectSaveResultDto::from(result);
    emit(Event::Project(EventProject::ProjectSaved {
        result: result.clone(),
    }));
    Ok(result)
}

#[tauri::command]
pub fn flush_project(
    app: AppHandle,
    application: State<ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
) -> Result<ProjectSaveResultDto, CommandError> {
    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "flushProject",
        "Flushing project"
    );
    let result = application
        .flush_project_for_application(project_instance_id, operation_id)
        .map_err(map_application_project_lifecycle_error)?;
    let result = ProjectSaveResultDto::from(result);
    emit_project_event(
        &app,
        Event::Project(EventProject::ProjectSaved {
            result: result.clone(),
        }),
    );
    Ok(result)
}

/// 新建项目（清空当前状态）
#[tauri::command]
pub fn new_project(
    app: AppHandle,
    application: State<'_, ApplicationState>,
    watcher: State<ProjectWatcherState>,
) -> Result<(), CommandError> {
    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "newProject",
        "Creating new project"
    );

    application
        .clear_project_for_application()
        .map_err(map_application_project_lifecycle_error)?;
    watcher.stop();
    emit_project_event(&app, Event::Project(EventProject::ProjectCleared));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::application_event::{
        LifecycleInvalidationDto, LifecycleMutationKindDto, LifecycleMutationPhaseDto,
        LifecycleRecoveryDto,
    };
    use yss_project_filesystem::ProjectFilesystemError;
    use yss_project_identity::OperationId;
    use yss_project_model::ProjectData;

    fn lifecycle_result(
        outcome: LifecycleMutationOutcomeDto,
        recovery_action: &str,
    ) -> LifecycleMutationResultDto {
        LifecycleMutationResultDto {
            operation_id: OperationId::new(),
            kind: LifecycleMutationKindDto::Delete,
            old_project_instance_id: Some("old-project".into()),
            new_project_instance_id: None,
            phase: LifecycleMutationPhaseDto::AuthorityCommitted,
            outcome,
            record: None,
            path: Some("C:/project".into()),
            recovery: Some(LifecycleRecoveryDto {
                required: true,
                action: recovery_action.into(),
                path: Some("C:/project".into()),
                identity: None,
            }),
            invalidation: LifecycleInvalidationDto {
                project: true,
                registry: true,
            },
        }
    }

    #[test]
    fn lifecycle_event_panic_preserves_direct_committed_receipt() {
        let result = lifecycle_result(
            LifecycleMutationOutcomeDto::RegistryPending,
            "removeRegistryRecord",
        );
        let direct = result.clone();

        publish_lifecycle_result_with(&result, |_| panic!("injected lifecycle emitter panic"));

        assert_eq!(direct, result);
    }

    #[test]
    fn lifecycle_errors_map_to_expected_or_diagnosed_command_errors() {
        let invalid_path = map_project_lifecycle_error(ProjectLifecycleError::InvalidPath);
        assert_eq!(invalid_path.code(), "invalid_path");
        assert!(invalid_path.incident_id().is_none());

        let not_found = map_project_lifecycle_error(ProjectLifecycleError::ProjectNotFound);
        assert_eq!(not_found.code(), "project_not_found");
        assert!(not_found.incident_id().is_none());

        let authority = map_project_lifecycle_error(ProjectLifecycleError::AuthorityFailed(
            ProjectFilesystemError::StaleProjectLifecycle {
                message: "test authority failure".into(),
            },
        ));
        assert_eq!(authority.code(), "stale_project_lifecycle");
        assert!(authority.incident_id().is_none());

        let diagnosed = map_project_lifecycle_error(ProjectLifecycleError::LoadFailed(
            ProjectFilesystemError::TransactionPrepareFailed {
                message: "test load failure".into(),
            },
        ));
        assert_eq!(diagnosed.code(), "load_project_failed");
        assert!(diagnosed.incident_id().is_some());
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
