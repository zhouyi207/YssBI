use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

use crate::events::{
    LifecycleInvalidation, LifecycleRecovery, LifecycleRecoveryAction,
    ProjectLifecycleApplicationEvent, ProjectLifecycleKind, ProjectLifecycleOutcome,
    ProjectLifecyclePhase,
};
use crate::project::query::ProjectActivation;
use crate::session::{
    ApplicationSession, ApplicationSessionRefreshError, ApplicationState, ProjectReplacement,
    SessionCaptureError, SessionRevalidationError,
};
use yss_project::ProjectOperationError;
use yss_project::ProjectState;
use yss_project_identity::OperationId;
use yss_project_identity::ProjectInstanceId;
use yss_project_registry::{ProjectRegistry, normalize_existing_path};
use yss_project_registry_contract::ProjectRecord;

use super::registry::ProjectManagement;

#[derive(Debug, Error)]
pub enum ProjectLifecycleError {
    #[error("project path is invalid")]
    InvalidPath,
    #[error("project load failed")]
    LoadFailed(#[source] ProjectOperationError),
    #[error("project lifecycle authority operation failed")]
    AuthorityFailed(#[source] ProjectOperationError),
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
        let captured = self.capture_session()?;
        let path = normalize_existing_path(path).map_err(|_| ProjectLifecycleError::InvalidPath)?;
        // A rejected file has not started a session replacement. Preserve existing admission,
        // drafts and results until Project has validated the target's activation data.
        let prepared = captured
            .project()
            .prepare_project_activation(Some(Path::new(&path)))
            .map_err(ProjectLifecycleError::LoadFailed)?;
        self.revalidate_captured_session(&captured)
            .map_err(ApplicationProjectLifecycleError::SessionChanged)?;
        let replacement = begin_replacement(self, &captured)?;
        let result = replacement
            .project()
            .activate_prepared_project(prepared)
            .map(|session| ProjectActivation {
                path,
                project_instance_id: session.instance_id,
            })
            .map_err(ProjectLifecycleError::LoadFailed);
        // Project rechecks files under its final lease. If that check fails after draining,
        // rebuild admission from the still-current authority instead of abandoning the worker.
        finish_replacement(self, replacement)?;
        result.map_err(Into::into)
    }

    pub fn clear_project_for_application(
        &self,
        expected_instance_id: &ProjectInstanceId,
    ) -> Result<(), ApplicationProjectLifecycleError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != expected_instance_id {
            return Err(ApplicationProjectLifecycleError::SessionChanged(
                SessionRevalidationError::Changed,
            ));
        }
        let replacement = begin_replacement(self, &captured)?;
        if replacement.project().project_instance_id() != expected_instance_id.as_str() {
            finish_replacement(self, replacement)?;
            return Err(ApplicationProjectLifecycleError::SessionChanged(
                SessionRevalidationError::Changed,
            ));
        }
        clear_project(replacement.project())?;
        finish_replacement(self, replacement)?;
        Ok(())
    }

    pub async fn save_project_as_for_application(
        &self,
        projects: &ProjectManagement,
        destination: &Path,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ProjectLifecycleApplicationEvent, ApplicationProjectLifecycleError> {
        let captured = self.capture_session()?;
        let result = save_project_as(
            captured.project(),
            &projects.registry,
            destination,
            project_instance_id,
            operation_id,
        )
        .await?;
        if result.outcome == ProjectLifecycleOutcome::Committed {
            self.rebuild_application_session(&captured)
                .map_err(ApplicationProjectLifecycleError::SessionRefresh)?;
        } else {
            self.revalidate_captured_session(&captured)
                .map_err(ApplicationProjectLifecycleError::SessionChanged)?;
        }
        Ok(result)
    }

    pub async fn create_project_for_application(
        &self,
        projects: &ProjectManagement,
        name: &str,
        path: &Path,
        operation_id: OperationId,
    ) -> Result<ProjectLifecycleApplicationEvent, ApplicationProjectLifecycleError> {
        let captured = self.capture_session()?;
        let result = create_project(
            captured.project(),
            &projects.registry,
            name,
            path,
            operation_id,
        )
        .await?;
        if result.invalidation.project {
            self.rebuild_application_session(&captured)
                .map_err(ApplicationProjectLifecycleError::SessionRefresh)?;
        }
        Ok(result)
    }
}

fn begin_replacement(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
) -> Result<ProjectReplacement, ApplicationProjectLifecycleError> {
    application
        .begin_project_replacement(captured)
        .map_err(|_error| {
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
                LifecycleRecoveryAction::RegisterDestination,
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
            LifecycleRecoveryAction::RegisterDestination,
            false,
        ),
    };
    Ok(result)
}
#[allow(
    clippy::too_many_arguments,
    reason = "a lifecycle failure event is published as one complete immutable fact envelope"
)]
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

#[cfg(test)]
mod tests;
