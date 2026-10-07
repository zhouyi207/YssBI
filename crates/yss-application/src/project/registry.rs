//! Process-wide project registration and picker-task lifecycle.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use yss_project_progress::{
    ProjectProgressSink, ProjectTaskCancellation, ProjectTaskCancellationRegistry,
};
use yss_project_registry::{
    CleanupInvalidProjectsResult, ProjectRegistry, ProjectRegistryError, ScanProjectsResult,
};
use yss_project_registry_contract::{ProjectRecord, ProjectRegistryStore};

pub struct ProjectManagement {
    pub(super) registry: ProjectRegistry,
    tasks: ProjectTaskCancellationRegistry,
}

impl ProjectManagement {
    pub fn new(store: Arc<dyn ProjectRegistryStore>, path: PathBuf) -> Self {
        Self {
            registry: ProjectRegistry::new(store, path),
            tasks: ProjectTaskCancellationRegistry::new(),
        }
    }

    pub fn path(&self) -> &Path {
        self.registry.path()
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectRecord>, ProjectRegistryError> {
        self.registry.list_projects().await
    }

    pub async fn register_project(
        &self,
        name: &str,
        path: &str,
    ) -> Result<ProjectRecord, ProjectRegistryError> {
        self.registry.register_project(name, path).await
    }

    pub async fn remove_project(&self, id: &str) -> Result<(), ProjectRegistryError> {
        self.registry.remove_project(id).await
    }

    pub async fn toggle_favorite(&self, id: &str) -> Result<bool, ProjectRegistryError> {
        self.registry.toggle_favorite(id).await
    }

    pub fn cancel_picker_task(&self) {
        self.tasks.cancel_active();
    }

    pub async fn scan_directory(
        &self,
        directory: &str,
        progress: &dyn ProjectProgressSink,
    ) -> Result<ScanProjectsResult, ProjectRegistryError> {
        let task = PickerTask::begin(&self.tasks);
        self.registry
            .scan_directory(directory, Some(progress), task.cancellation.clone())
            .await
    }

    pub async fn cleanup_invalid_projects(
        &self,
        progress: &dyn ProjectProgressSink,
    ) -> Result<CleanupInvalidProjectsResult, ProjectRegistryError> {
        let task = PickerTask::begin(&self.tasks);
        self.registry
            .cleanup_invalid_projects(Some(progress), task.cancellation.clone())
            .await
    }
}

// A dropped command future must release admission just like a completed operation.
struct PickerTask<'a> {
    tasks: &'a ProjectTaskCancellationRegistry,
    cancellation: ProjectTaskCancellation,
}

impl<'a> PickerTask<'a> {
    fn begin(tasks: &'a ProjectTaskCancellationRegistry) -> Self {
        Self {
            tasks,
            cancellation: tasks.begin(),
        }
    }
}

impl Drop for PickerTask<'_> {
    fn drop(&mut self) {
        self.tasks.end(&self.cancellation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::{Context, Poll, Waker};
    use yss_project_identity::ProjectRegistrationId;
    use yss_project_progress::ProjectProgress;
    use yss_project_registry_contract::{ProjectRegistryStoreError, ProjectRegistryStoreFuture};

    #[derive(Default)]
    struct PausedStore(AtomicBool);

    impl ProjectRegistryStore for PausedStore {
        fn load(
            &self,
        ) -> ProjectRegistryStoreFuture<'_, Result<Box<[ProjectRecord]>, ProjectRegistryStoreError>>
        {
            Box::pin(std::future::poll_fn(|_| {
                if self.0.load(Ordering::Acquire) {
                    Poll::Ready(Ok(Vec::new().into_boxed_slice()))
                } else {
                    Poll::Pending
                }
            }))
        }

        fn upsert(
            &self,
            _: &ProjectRecord,
        ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>> {
            unreachable!("the empty registry has no records to write")
        }

        fn remove(
            &self,
            _: &ProjectRegistrationId,
        ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>> {
            unreachable!("the empty registry has no records to remove")
        }
    }

    struct Progress;
    impl ProjectProgressSink for Progress {
        fn publish(&self, _: ProjectProgress) {}
    }

    #[test]
    fn picker_cancellation_targets_current_task_after_an_old_future_is_dropped() {
        let store = Arc::new(PausedStore::default());
        let projects = ProjectManagement::new(store.clone(), PathBuf::from("unused.sqlite"));
        let mut context = Context::from_waker(Waker::noop());
        let mut old = Box::pin(projects.cleanup_invalid_projects(&Progress));
        assert!(old.as_mut().poll(&mut context).is_pending());
        let mut current = Box::pin(projects.cleanup_invalid_projects(&Progress));
        assert!(current.as_mut().poll(&mut context).is_pending());
        drop(old);
        projects.cancel_picker_task();
        store.0.store(true, Ordering::Release);
        assert!(matches!(
            current.as_mut().poll(&mut context),
            Poll::Ready(Err(ProjectRegistryError::Cancelled))
        ));
        drop(current);
        let mut next = Box::pin(projects.cleanup_invalid_projects(&Progress));
        assert!(matches!(
            next.as_mut().poll(&mut context),
            Poll::Ready(Ok(CleanupInvalidProjectsResult { removed: 0 }))
        ));
    }
}
