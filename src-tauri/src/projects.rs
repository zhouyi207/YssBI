//! Desktop persistence and file-watcher selection for project services.

use std::path::PathBuf;
use std::sync::Arc;
use yss_application::project_lifecycle::ProjectManagement;

pub(super) async fn initialize_registry(
    app_dir: PathBuf,
) -> Result<ProjectManagement, Box<dyn std::error::Error>> {
    let store = yss_project_registry_sqlite::SqliteProjectRegistryStore::connect(app_dir).await?;
    let path = store.path().to_path_buf();
    Ok(ProjectManagement::new(Arc::new(store), path))
}

pub(super) fn watcher() -> yss_project_watcher::ProjectWatcherState {
    yss_project_watcher::ProjectWatcherState::new(Arc::new(
        yss_project_watcher_notify::NotifyProjectFileWatcher::new(),
    ))
}
