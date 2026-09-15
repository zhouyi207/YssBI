use thiserror::Error;
use yss_filesystem::change::FilesystemChange;
use yss_project::ProjectIndexInvalidation;
use yss_project::ProjectOperationError;
use yss_project_identity::ProjectInstanceId;

use crate::session::{ApplicationState, SessionCaptureError};

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
        ProjectOperationError,
    ),
    #[error("captured application session changed during watcher reconciliation")]
    SessionChanged,
}

impl ApplicationState {
    pub fn reconcile_project_change(
        &self,
        project_instance_id: &ProjectInstanceId,
        change: FilesystemChange,
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
