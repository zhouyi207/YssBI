use thiserror::Error;
use yss_project_change::{ProjectChange, ProjectIndexInvalidation};
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::ProjectInstanceId;

use super::execution::session_slot::{ApplicationState, SessionCaptureError};

#[derive(Debug, Error)]
pub enum ApplicationProjectWatchError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("watched project identity is stale")]
    ProjectIdentityMismatch,
    #[error("project index reconciliation failed")]
    Reconciliation(
        #[source]
        #[from]
        ProjectFilesystemError,
    ),
    #[error("captured application session changed during watcher reconciliation")]
    SessionChanged,
}

impl ApplicationState {
    pub fn reconcile_project_change(
        &self,
        project_instance_id: &ProjectInstanceId,
        change: ProjectChange,
    ) -> Result<Option<ProjectIndexInvalidation>, ApplicationProjectWatchError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project_instance_id {
            return Err(ApplicationProjectWatchError::ProjectIdentityMismatch);
        }
        let event = captured
            .project()
            .reconcile_project_change(project_instance_id, change)?;
        self.revalidate_captured_session(&captured)
            .map_err(|_| ApplicationProjectWatchError::SessionChanged)?;
        Ok(event)
    }
}
