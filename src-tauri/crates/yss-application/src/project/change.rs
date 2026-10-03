use std::sync::{Arc, Mutex, PoisonError};

use thiserror::Error;
use yss_filesystem::change::FilesystemChange;
use yss_filesystem::watcher::{ChangeSink, WatcherError, WatcherState};
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
    pub(crate) fn watch_project_changes(
        &self,
        watcher: &Mutex<WatcherState>,
        path: &str,
        project_instance_id: &ProjectInstanceId,
        sink: Arc<dyn ChangeSink>,
    ) -> Result<(), WatcherError> {
        let watcher = watcher.lock().unwrap_or_else(PoisonError::into_inner);
        // Serialize the identity check with watcher replacement, not with the
        // Application slot: draining may call back into project reconciliation.
        let current = self.capture_session();
        let result = match &current {
            Ok(current) if current.project_instance_id() == project_instance_id => {
                watcher.watch(yss_project::project_root_from_path(path), sink)
            }
            _ => Ok(()),
        };
        drop(watcher);
        result
    }

    pub(crate) fn stop_project_watcher(&self, watcher: &Mutex<WatcherState>) {
        let watcher = watcher.lock().unwrap_or_else(PoisonError::into_inner);
        let current = self.capture_session();
        if let Ok(current) = &current
            && current.project().get_path().is_none()
        {
            watcher.stop();
        }
        drop(watcher);
    }

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
