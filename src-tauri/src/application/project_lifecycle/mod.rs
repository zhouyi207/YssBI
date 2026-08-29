use std::future::Future;
use std::path::Path;

use thiserror::Error;

use super::events::{
    LifecycleInvalidation, LifecycleRecovery, LifecycleRecoveryAction,
    ProjectLifecycleApplicationEvent, ProjectLifecycleKind, ProjectLifecycleOutcome,
    ProjectLifecyclePhase,
};
use crate::application::execution::session_slot::{
    ApplicationSessionRefreshError, ApplicationState, ProjectReplacement, SessionCaptureError,
    SessionRevalidationError,
};
use crate::application::project_query::ProjectActivation;
use crate::project::OperationId;
use crate::project::{
    ProjectFilesystemError, ProjectInstanceId, ProjectRecord, ProjectRegistry, ProjectState,
    normalize_existing_path,
};

#[derive(Debug, Error)]
pub enum ProjectLifecycleError {
    #[error("project path is invalid")]
    InvalidPath,
    #[error("registered project was not found")]
    ProjectNotFound,
    #[error("project load failed")]
    LoadFailed(#[source] ProjectFilesystemError),
    #[error("project lifecycle authority operation failed")]
    AuthorityFailed(#[source] ProjectFilesystemError),
    #[error("project registry lookup failed")]
    RegistryLookupFailed(#[source] ProjectRegistryFailure),
}

pub struct ProjectRegistryFailure {
    diagnostic: Box<str>,
}

impl ProjectRegistryFailure {
    fn new(diagnostic: impl std::fmt::Display) -> Self {
        Self {
            diagnostic: diagnostic.to_string().into_boxed_str(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ApplicationProjectLifecycleError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error(transparent)]
    Lifecycle(#[from] ProjectLifecycleError),
    #[error("captured application session changed during project lifecycle operation")]
    SessionChanged(#[source] SessionRevalidationError),
    #[error("application session refresh failed")]
    SessionRefresh(#[source] ApplicationSessionRefreshError),
}

impl ApplicationState {
    pub fn load_project_for_application(
        &self,
        path: &str,
    ) -> Result<ProjectActivation, ApplicationProjectLifecycleError> {
        let replacement = begin_replacement(self)?;
        let result = load_project(replacement.project(), path)?;
        finish_replacement(self, replacement)?;
        Ok(result)
    }

    pub fn clear_project_for_application(&self) -> Result<(), ApplicationProjectLifecycleError> {
        let replacement = begin_replacement(self)?;
        clear_project(replacement.project())?;
        finish_replacement(self, replacement)?;
        Ok(())
    }

    pub async fn save_project_as_for_application(
        &self,
        registry: &ProjectRegistry,
        destination: &Path,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ProjectLifecycleApplicationEvent, ApplicationProjectLifecycleError> {
        let captured = self.capture_session()?;
        let result = save_project_as(
            captured.project(),
            registry,
            destination,
            project_instance_id,
            operation_id,
        )
        .await?;
        if result.outcome == ProjectLifecycleOutcome::Committed {
            self.refresh_current_project()
                .map_err(ApplicationProjectLifecycleError::SessionRefresh)?;
        } else {
            self.revalidate_captured_session(&captured)
                .map_err(ApplicationProjectLifecycleError::SessionChanged)?;
        }
        Ok(result)
    }

    pub async fn create_project_for_application(
        &self,
        registry: &ProjectRegistry,
        name: &str,
        path: &Path,
        operation_id: OperationId,
    ) -> Result<ProjectLifecycleApplicationEvent, ApplicationProjectLifecycleError> {
        let captured = self.capture_session()?;
        let result = create_project(captured.project(), registry, name, path, operation_id).await?;
        if result.invalidation.project {
            self.refresh_current_project()
                .map_err(ApplicationProjectLifecycleError::SessionRefresh)?;
        }
        Ok(result)
    }

    pub async fn delete_registered_project_for_application(
        &self,
        registry: &ProjectRegistry,
        id: &str,
        expected_active_instance_id: Option<ProjectInstanceId>,
        operation_id: OperationId,
    ) -> Result<ProjectLifecycleApplicationEvent, ApplicationProjectLifecycleError> {
        let captured = self.capture_session()?;
        let replacement = expected_active_instance_id
            .as_ref()
            .filter(|expected| *expected == captured.project_instance_id())
            .map(|_| begin_replacement(self))
            .transpose()?;
        let project = replacement
            .as_ref()
            .map_or_else(|| captured.project(), ProjectReplacement::project);
        let result = delete_registered_project(
            project,
            registry,
            id,
            expected_active_instance_id,
            operation_id,
        )
        .await?;
        if let Some(replacement) = replacement {
            finish_replacement(self, replacement)?;
        } else if result.invalidation.project {
            self.refresh_current_project()
                .map_err(ApplicationProjectLifecycleError::SessionRefresh)?;
        } else {
            self.revalidate_captured_session(&captured)
                .map_err(ApplicationProjectLifecycleError::SessionChanged)?;
        }
        Ok(result)
    }

    pub fn flush_project_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<crate::project::project_writers::ProjectSaveResult, ApplicationProjectLifecycleError>
    {
        let captured = self.capture_session()?;
        let result = captured
            .project()
            .flush_project_documents(&project_instance_id, operation_id)
            .map_err(ProjectLifecycleError::AuthorityFailed)?;
        self.revalidate_captured_session(&captured)
            .map_err(ApplicationProjectLifecycleError::SessionChanged)?;
        Ok(result)
    }
}

fn begin_replacement(
    application: &ApplicationState,
) -> Result<ProjectReplacement, ApplicationProjectLifecycleError> {
    application.begin_project_replacement().map_err(|_error| {
        ApplicationProjectLifecycleError::SessionRefresh(
            ApplicationSessionRefreshError::Replacement,
        )
    })
}

fn finish_replacement(
    application: &ApplicationState,
    replacement: ProjectReplacement,
) -> Result<(), ApplicationProjectLifecycleError> {
    application
        .finish_project_replacement(replacement)
        .map_err(ApplicationProjectLifecycleError::SessionRefresh)
}

impl std::fmt::Debug for ProjectRegistryFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectRegistryFailure")
            .field("diagnostic", &self.diagnostic)
            .finish()
    }
}

impl std::fmt::Display for ProjectRegistryFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("project registry operation failed")
    }
}

impl std::error::Error for ProjectRegistryFailure {}

pub fn load_project(
    state: &ProjectState,
    path: &str,
) -> Result<ProjectActivation, ProjectLifecycleError> {
    let path = normalize_existing_path(path).map_err(|_| ProjectLifecycleError::InvalidPath)?;
    let session = state
        .activate_project_from_path(Path::new(&path))
        .map_err(ProjectLifecycleError::LoadFailed)?;
    state
        .get_data()
        .map_err(ProjectLifecycleError::LoadFailed)?;

    Ok(ProjectActivation {
        path,
        project_instance_id: session.instance_id,
        activation_revision: state.activation_revision(),
    })
}

pub fn clear_project(state: &ProjectState) -> Result<(), ProjectLifecycleError> {
    state
        .clear_project()
        .map(|_| ())
        .map_err(ProjectLifecycleError::AuthorityFailed)
}

pub async fn save_project_as(
    state: &ProjectState,
    registry: &ProjectRegistry,
    destination: &Path,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
) -> Result<ProjectLifecycleApplicationEvent, ProjectLifecycleError> {
    let prepared = state
        .save_project_as_transaction(&project_instance_id, destination, operation_id)
        .map_err(ProjectLifecycleError::AuthorityFailed)?;
    let metadata_path = prepared.metadata_path.to_string_lossy().into_owned();
    let project_name = prepared
        .prepared_activation
        .data
        .metadata
        .project_name
        .clone();
    let record = match registry
        .register_project(&project_name, &metadata_path)
        .await
    {
        Ok(record) => record,
        Err(_) => {
            return Ok(lifecycle_failure_result(
                operation_id,
                ProjectLifecycleKind::SaveAs,
                Some(project_instance_id),
                ProjectLifecyclePhase::DestinationCommitted,
                ProjectLifecycleOutcome::RegistryFailed,
                None,
                metadata_path,
                LifecycleRecoveryAction::RemoveRegistryRecord,
                false,
            ));
        }
    };
    let session = match state.activate_prepared_project(prepared.prepared_activation) {
        Ok(session) => session,
        Err(_) => {
            return Ok(lifecycle_failure_result(
                operation_id,
                ProjectLifecycleKind::SaveAs,
                Some(project_instance_id),
                ProjectLifecyclePhase::RegistryCommitted,
                ProjectLifecycleOutcome::ActivationFailed,
                Some(record),
                metadata_path,
                LifecycleRecoveryAction::ActivateDestination,
                true,
            ));
        }
    };

    Ok(ProjectLifecycleApplicationEvent {
        operation_id,
        kind: ProjectLifecycleKind::SaveAs,
        old_project_instance_id: Some(project_instance_id),
        new_project_instance_id: Some(session.instance_id),
        phase: ProjectLifecyclePhase::AuthorityCommitted,
        outcome: ProjectLifecycleOutcome::Committed,
        record: Some(record),
        path: Some(metadata_path.into_boxed_str()),
        recovery: None,
        invalidation: LifecycleInvalidation {
            project: true,
            registry: true,
        },
    })
}

pub async fn create_project(
    state: &ProjectState,
    registry: &ProjectRegistry,
    name: &str,
    destination: &Path,
    operation_id: OperationId,
) -> Result<ProjectLifecycleApplicationEvent, ProjectLifecycleError> {
    let created = state
        .create_project_transaction(name, destination, operation_id)
        .map_err(ProjectLifecycleError::AuthorityFailed)?;
    let metadata_path = created.metadata_path.to_string_lossy().into_owned();
    let result = match registry
        .register_project(&created.project_name, &metadata_path)
        .await
    {
        Ok(record) => ProjectLifecycleApplicationEvent {
            operation_id,
            kind: ProjectLifecycleKind::Create,
            old_project_instance_id: None,
            new_project_instance_id: None,
            phase: ProjectLifecyclePhase::RegistryCommitted,
            outcome: ProjectLifecycleOutcome::Committed,
            record: Some(record),
            path: Some(metadata_path.into_boxed_str()),
            recovery: None,
            invalidation: LifecycleInvalidation {
                project: false,
                registry: true,
            },
        },
        Err(_) => lifecycle_failure_result(
            operation_id,
            ProjectLifecycleKind::Create,
            None,
            ProjectLifecyclePhase::DestinationCommitted,
            ProjectLifecycleOutcome::RegistryFailed,
            None,
            metadata_path,
            LifecycleRecoveryAction::RemoveRegistryRecord,
            false,
        ),
    };
    Ok(result)
}

pub async fn delete_registered_project(
    state: &ProjectState,
    registry: &ProjectRegistry,
    id: &str,
    expected_active_instance_id: Option<ProjectInstanceId>,
    operation_id: OperationId,
) -> Result<ProjectLifecycleApplicationEvent, ProjectLifecycleError> {
    let record = registry
        .fetch_by_id(id)
        .await
        .map_err(|error| {
            ProjectLifecycleError::RegistryLookupFailed(ProjectRegistryFailure::new(error))
        })?
        .ok_or(ProjectLifecycleError::ProjectNotFound)?;

    let Some(identity) = record.deletion_identity().cloned() else {
        return delete_registry_only_record(
            registry,
            record,
            expected_active_instance_id.is_some(),
            operation_id,
        )
        .await;
    };

    let prepared = state
        .prepare_project_deletion(
            Path::new(&record.path),
            Some(&identity),
            expected_active_instance_id.as_ref(),
        )
        .map_err(ProjectLifecycleError::AuthorityFailed)?;
    let post_activation_failed = prepared.post_activation_failed();
    let deleted = state.commit_project_deletion(prepared);
    let registry_removed = if post_activation_failed {
        false
    } else {
        catch_future_unwind(async {
            #[cfg(test)]
            run_before_registry_remove_test_hook();
            registry.remove_project(&record.id).await
        })
        .await
        .is_ok_and(|result| result.is_ok())
    };

    let recovery = (!registry_removed).then(|| LifecycleRecovery {
        required: true,
        action: LifecycleRecoveryAction::RemoveRegistryRecord,
        path: None,
        identity: Some(identity.as_str().to_owned().into_boxed_str()),
    });
    let project_invalidated = deleted.cleared_project_instance_id.is_some();

    Ok(ProjectLifecycleApplicationEvent {
        operation_id,
        kind: ProjectLifecycleKind::Delete,
        old_project_instance_id: deleted.cleared_project_instance_id,
        new_project_instance_id: None,
        phase: ProjectLifecyclePhase::AuthorityCommitted,
        outcome: if registry_removed {
            ProjectLifecycleOutcome::Committed
        } else {
            ProjectLifecycleOutcome::RegistryPending
        },
        record: Some(record),
        path: Some(
            deleted
                .deleted_root
                .as_path()
                .to_string_lossy()
                .into_owned()
                .into_boxed_str(),
        ),
        recovery,
        invalidation: LifecycleInvalidation {
            project: project_invalidated,
            registry: true,
        },
    })
}

async fn delete_registry_only_record(
    registry: &ProjectRegistry,
    record: ProjectRecord,
    active_delete_rejected: bool,
    operation_id: OperationId,
) -> Result<ProjectLifecycleApplicationEvent, ProjectLifecycleError> {
    let remove_result = if active_delete_rejected {
        Err(())
    } else {
        registry.remove_project(&record.id).await.map_err(|_| ())
    };
    let registry_changed = remove_result.is_ok();

    Ok(ProjectLifecycleApplicationEvent {
        operation_id,
        kind: ProjectLifecycleKind::RegistryCleanup,
        old_project_instance_id: None,
        new_project_instance_id: None,
        phase: ProjectLifecyclePhase::RegistryCommitted,
        outcome: if registry_changed {
            ProjectLifecycleOutcome::Committed
        } else {
            ProjectLifecycleOutcome::RegistryFailed
        },
        record: Some(record),
        path: None,
        recovery: remove_result.err().map(|_| LifecycleRecovery {
            required: true,
            action: LifecycleRecoveryAction::CleanupRegistry,
            path: None,
            identity: None,
        }),
        invalidation: LifecycleInvalidation {
            project: false,
            registry: true,
        },
    })
}

fn lifecycle_failure_result(
    operation_id: OperationId,
    kind: ProjectLifecycleKind,
    old_project_instance_id: Option<ProjectInstanceId>,
    phase: ProjectLifecyclePhase,
    outcome: ProjectLifecycleOutcome,
    record: Option<ProjectRecord>,
    path: String,
    action: LifecycleRecoveryAction,
    project_invalidation: bool,
) -> ProjectLifecycleApplicationEvent {
    ProjectLifecycleApplicationEvent {
        operation_id,
        kind,
        old_project_instance_id,
        new_project_instance_id: None,
        phase,
        outcome,
        record,
        path: Some(path.clone().into_boxed_str()),
        recovery: Some(LifecycleRecovery {
            required: true,
            action,
            path: Some(path.into_boxed_str()),
            identity: None,
        }),
        invalidation: LifecycleInvalidation {
            project: project_invalidation,
            registry: true,
        },
    }
}

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

#[cfg(test)]
static BEFORE_REGISTRY_REMOVE_HOOK: std::sync::Mutex<
    Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
> = std::sync::Mutex::new(None);

#[cfg(test)]
fn set_before_registry_remove_test_hook(hook: Option<std::sync::Arc<dyn Fn() + Send + Sync>>) {
    *BEFORE_REGISTRY_REMOVE_HOOK.lock().unwrap() = hook;
}

#[cfg(test)]
fn run_before_registry_remove_test_hook() {
    let hook = BEFORE_REGISTRY_REMOVE_HOOK.lock().unwrap().clone();
    if let Some(hook) = hook {
        hook();
    }
}

#[cfg(all(test, any()))]
mod tests;
