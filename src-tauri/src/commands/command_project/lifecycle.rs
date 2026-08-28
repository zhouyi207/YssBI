use crate::application::project_lifecycle::{self, ProjectLifecycleError};
use crate::application::project_watcher::{
    ObservedProjectFileChange, ProjectFileChangeSink, ProjectWatcherError, ProjectWatcherState,
};
use crate::error::CommandError;
use crate::event::{
    Event, EventProject, EventResource, LifecycleMutationOutcomeDto, LifecycleMutationResultDto,
    ProjectActivationResultDto, emit_project_event, emit_project_event_result,
};
use crate::project::OperationId;
use crate::project::project_writers::ProjectSaveResultDto;
use crate::project::{
    ProjectDomainEvent, ProjectInstanceId, ProjectRegistry, ProjectState, ProjectWatchError,
};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use tauri::{AppHandle, State};

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
        ProjectLifecycleError::AuthorityFailed(source) => CommandError::from(source),
        ProjectLifecycleError::RegistryLookupFailed(source) => CommandError::internal(source),
    }
}

fn start_project_watcher(
    app: &AppHandle,
    project: &ProjectState,
    watcher: &ProjectWatcherState,
    path: &str,
    project_instance_id: &ProjectInstanceId,
) {
    let sink = Arc::new(ProjectEventWatcherSink {
        app: app.clone(),
        project: project.clone(),
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
    project: ProjectState,
    project_instance_id: ProjectInstanceId,
    version: Mutex<u64>,
}

impl ProjectFileChangeSink for ProjectEventWatcherSink {
    fn publish(&self, change: ObservedProjectFileChange) {
        let domain_event = match self
            .project
            .reconcile_file_change(&self.project_instance_id, change.change)
        {
            Ok(event) => event,
            Err(ProjectWatchError::Irrelevant) => return,
            Err(error) => {
                tracing::warn!(
                    target: "yssbi::project::watcher",
                    diagnostic_domain = "system",
                    diagnostic_event = "projectIndexRefreshFailed",
                    error_kind = project_watch_error_kind(&error),
                    "Failed to reconcile watched project file change"
                );
                return;
            }
        };

        let ProjectDomainEvent::ProjectIndexInvalidated {
            project_instance_id,
        } = domain_event;
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

fn project_watch_error_kind(error: &ProjectWatchError) -> &'static str {
    match error {
        ProjectWatchError::Irrelevant => "irrelevant",
        ProjectWatchError::Reconciliation(_) => "reconciliation_failed",
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
    application: State<'_, crate::application::execution::ApplicationState>,
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

    let result = project_lifecycle::load_project(state.inner(), &path)
        .map_err(map_project_lifecycle_error)?;
    application
        .refresh_current_project()
        .map_err(|error| CommandError::diagnosed("project_session_refresh_failed", error))?;

    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "projectLoaded",
        project_instance_id = result.project_instance_id.as_str(),
        "Project loaded"
    );

    let project_instance_id = ProjectInstanceId::from_existing(result.project_instance_id.clone());
    start_project_watcher(
        &app,
        state.inner(),
        &watcher,
        &result.path,
        &project_instance_id,
    );
    emit_project_loaded(&app, result.clone());
    Ok(result)
}

/// 将当前项目另存为新目录（完整复制 events/functions/database 等）。
#[tauri::command]
pub async fn save_project_as(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
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

    let result = project_lifecycle::save_project_as(
        state.inner(),
        registry.inner(),
        Path::new(&path),
        project_instance_id,
        operation_id,
    )
    .await
    .map_err(map_project_lifecycle_error)?;
    publish_lifecycle_result(&app, &result);
    if result.outcome == LifecycleMutationOutcomeDto::Committed {
        application
            .refresh_current_project()
            .map_err(|error| CommandError::diagnosed("project_session_refresh_failed", error))?;
        if let (Some(metadata_path), Some(project_instance_id)) = (
            result.path.as_deref(),
            result.new_project_instance_id.as_deref(),
        ) {
            let project_instance_id = ProjectInstanceId::from_existing(project_instance_id.into());
            start_project_watcher(
                &app,
                state.inner(),
                &watcher,
                metadata_path,
                &project_instance_id,
            );
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
    let result = project_lifecycle::create_project(
        state.inner(),
        registry.inner(),
        &name,
        Path::new(&path),
        operation_id,
    )
    .await
    .map_err(map_project_lifecycle_error)?;
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
    application: State<'_, crate::application::execution::ApplicationState>,
    state: State<ProjectState>,
    watcher: State<ProjectWatcherState>,
) -> Result<(), CommandError> {
    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "newProject",
        "Creating new project"
    );

    project_lifecycle::clear_project(state.inner()).map_err(map_project_lifecycle_error)?;
    application
        .refresh_current_project()
        .map_err(|error| CommandError::diagnosed("project_session_refresh_failed", error))?;
    watcher.stop();
    emit_project_event(&app, Event::Project(EventProject::ProjectCleared));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{
        LifecycleInvalidationDto, LifecycleMutationKindDto, LifecycleMutationPhaseDto,
        LifecycleRecoveryDto,
    };
    use crate::project::OperationId;
    use crate::project::{ProjectData, ProjectFilesystemError};

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
