use crate::project::{ProjectFilesystemError, ProjectState};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use yss_project_identity::ProjectInstanceId;
use yss_project_layout::{PROJECT_METADATA_FILE, is_project_index_input_path};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectRelativePath(PathBuf);

impl ProjectRelativePath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub(crate) fn from_observed(path: PathBuf) -> Option<Self> {
        if path.as_os_str().is_empty()
            || path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return None;
        }
        Some(Self(path))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileChangeKind {
    Created,
    Modified,
    Removed,
    Renamed,
    WatcherError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChange {
    pub relative_path: ProjectRelativePath,
    pub kind: FileChangeKind,
}

impl FileChange {
    pub fn new(relative_path: ProjectRelativePath, kind: FileChangeKind) -> Self {
        Self {
            relative_path,
            kind,
        }
    }

    pub(crate) fn watcher_error() -> Self {
        Self::new(
            ProjectRelativePath::new(PROJECT_METADATA_FILE),
            FileChangeKind::WatcherError,
        )
    }

    pub fn is_relevant(&self) -> bool {
        self.kind == FileChangeKind::WatcherError
            || is_project_index_input_path(self.relative_path.as_path())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectDomainEvent {
    ProjectIndexInvalidated {
        project_instance_id: ProjectInstanceId,
    },
}

#[derive(Debug, Error)]
pub enum ProjectWatchError {
    #[error("project file change is not relevant to the project index")]
    Irrelevant,
    #[error("project index reconciliation failed")]
    Reconciliation(#[source] ProjectFilesystemError),
}

impl ProjectState {
    pub fn reconcile_file_change(
        &self,
        project: &ProjectInstanceId,
        change: FileChange,
    ) -> Result<ProjectDomainEvent, ProjectWatchError> {
        if !change.is_relevant() {
            return Err(ProjectWatchError::Irrelevant);
        }

        self.read_project_index(project)
            .map_err(ProjectWatchError::Reconciliation)?;
        Ok(ProjectDomainEvent::ProjectIndexInvalidated {
            project_instance_id: project.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures;
    use yss_project_model::ProjectData;

    #[test]
    fn file_change_keeps_neutral_path_and_kind_and_filters_unrelated_paths() {
        let change = FileChange::new(
            ProjectRelativePath::new("events/foo.yssbi-event"),
            FileChangeKind::Modified,
        );
        let unrelated = FileChange::new(
            ProjectRelativePath::new("README.md"),
            FileChangeKind::Modified,
        );

        assert_eq!(
            change.relative_path.as_path(),
            Path::new("events/foo.yssbi-event")
        );
        assert_eq!(change.kind, FileChangeKind::Modified);
        assert!(change.is_relevant());
        assert!(!unrelated.is_relevant());
    }

    #[test]
    fn project_reconciliation_returns_typed_index_invalidation() {
        let fixture = fixtures::TempProject::activate("watcher-reconcile", ProjectData::new());
        let project = ProjectInstanceId::from_existing(fixture.state().project_instance_id());
        let event = fixture
            .state()
            .reconcile_file_change(
                &project,
                FileChange::new(
                    ProjectRelativePath::new("metadata.yssbi"),
                    FileChangeKind::Modified,
                ),
            )
            .unwrap();

        assert_eq!(
            event,
            ProjectDomainEvent::ProjectIndexInvalidated {
                project_instance_id: project,
            }
        );
    }
}
