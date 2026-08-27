use std::future::Future;
use std::path::Path;

use thiserror::Error;

use crate::event::{
    LifecycleInvalidationDto, LifecycleMutationKindDto, LifecycleMutationOutcomeDto,
    LifecycleMutationPhaseDto, LifecycleMutationResultDto, LifecycleRecoveryDto,
    ProjectActivationResultDto,
};
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
    fn new(diagnostic: String) -> Self {
        Self {
            diagnostic: diagnostic.into_boxed_str(),
        }
    }
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
) -> Result<ProjectActivationResultDto, ProjectLifecycleError> {
    let path = normalize_existing_path(path).map_err(|_| ProjectLifecycleError::InvalidPath)?;
    let session = state
        .activate_project_from_path(Path::new(&path))
        .map_err(ProjectLifecycleError::LoadFailed)?;
    state
        .get_data()
        .map_err(ProjectLifecycleError::LoadFailed)?;

    Ok(ProjectActivationResultDto {
        path,
        project_instance_id: session.instance_id.to_string(),
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
) -> Result<LifecycleMutationResultDto, ProjectLifecycleError> {
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
                LifecycleMutationKindDto::SaveAs,
                Some(project_instance_id.to_string()),
                LifecycleMutationPhaseDto::DestinationCommitted,
                LifecycleMutationOutcomeDto::RegistryFailed,
                None,
                metadata_path,
                "registerDestination",
                false,
            ));
        }
    };
    let session = match state.activate_prepared_project(prepared.prepared_activation) {
        Ok(session) => session,
        Err(_) => {
            return Ok(lifecycle_failure_result(
                operation_id,
                LifecycleMutationKindDto::SaveAs,
                Some(project_instance_id.to_string()),
                LifecycleMutationPhaseDto::RegistryCommitted,
                LifecycleMutationOutcomeDto::ActivationFailed,
                Some(record),
                metadata_path,
                "activateDestination",
                true,
            ));
        }
    };

    Ok(LifecycleMutationResultDto {
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
    })
}

pub async fn create_project(
    state: &ProjectState,
    registry: &ProjectRegistry,
    name: &str,
    destination: &Path,
    operation_id: OperationId,
) -> Result<LifecycleMutationResultDto, ProjectLifecycleError> {
    let created = state
        .create_project_transaction(name, destination, operation_id)
        .map_err(ProjectLifecycleError::AuthorityFailed)?;
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
    Ok(result)
}

pub async fn delete_registered_project(
    state: &ProjectState,
    registry: &ProjectRegistry,
    id: &str,
    expected_active_instance_id: Option<ProjectInstanceId>,
    operation_id: OperationId,
) -> Result<LifecycleMutationResultDto, ProjectLifecycleError> {
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

    let recovery = (!registry_removed).then(|| LifecycleRecoveryDto {
        required: true,
        action: "removeRegistryRecord".into(),
        path: None,
        identity: Some(identity.as_str().to_owned()),
    });

    Ok(LifecycleMutationResultDto {
        operation_id,
        kind: LifecycleMutationKindDto::Delete,
        old_project_instance_id: deleted
            .cleared_project_instance_id
            .as_ref()
            .map(ToString::to_string),
        new_project_instance_id: None,
        phase: LifecycleMutationPhaseDto::AuthorityCommitted,
        outcome: if registry_removed {
            LifecycleMutationOutcomeDto::Committed
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
        recovery,
        invalidation: LifecycleInvalidationDto {
            project: deleted.cleared_project_instance_id.is_some(),
            registry: true,
        },
    })
}

async fn delete_registry_only_record(
    registry: &ProjectRegistry,
    record: ProjectRecord,
    active_delete_rejected: bool,
    operation_id: OperationId,
) -> Result<LifecycleMutationResultDto, ProjectLifecycleError> {
    let remove_result = if active_delete_rejected {
        Err(())
    } else {
        registry.remove_project(&record.id).await.map_err(|_| ())
    };
    let registry_changed = remove_result.is_ok();

    Ok(LifecycleMutationResultDto {
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
    })
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

#[cfg(test)]
mod tests;
