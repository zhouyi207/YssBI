//! Platform-neutral project-index change contracts.
//!
//! Filesystem observation and project-state reconciliation remain in their
//! respective adapters. This crate owns the safe relative-path invariant, the
//! change facts shared by those layers, and the typed invalidation result.

#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use yss_project_identity::ProjectInstanceId;
use yss_project_layout::is_project_index_input_path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectRelativePathError {
    Empty,
    Absolute,
    NonNormalComponent,
}

impl fmt::Display for ProjectRelativePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "project-relative path is empty",
            Self::Absolute => "project-relative path must not be absolute",
            Self::NonNormalComponent => {
                "project-relative path contains a current, parent, root, or prefix component"
            }
        })
    }
}

impl Error for ProjectRelativePathError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectRelativePath(PathBuf);

impl ProjectRelativePath {
    pub fn try_new(path: impl Into<PathBuf>) -> Result<Self, ProjectRelativePathError> {
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(ProjectRelativePathError::Empty);
        }
        if path.is_absolute() {
            return Err(ProjectRelativePathError::Absolute);
        }
        if path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ProjectRelativePathError::NonNormalComponent);
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProjectFileChangeKind {
    Created,
    Modified,
    Removed,
    Renamed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectFileChange {
    relative_path: ProjectRelativePath,
    kind: ProjectFileChangeKind,
}

impl ProjectFileChange {
    pub fn new(relative_path: ProjectRelativePath, kind: ProjectFileChangeKind) -> Self {
        Self {
            relative_path,
            kind,
        }
    }

    pub fn relative_path(&self) -> &ProjectRelativePath {
        &self.relative_path
    }

    pub fn kind(&self) -> ProjectFileChangeKind {
        self.kind
    }

    pub fn affects_project_index(&self) -> bool {
        is_project_index_input_path(self.relative_path.as_path())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectChange {
    File(ProjectFileChange),
    RescanRequired,
}

impl ProjectChange {
    pub fn file(relative_path: ProjectRelativePath, kind: ProjectFileChangeKind) -> Self {
        Self::File(ProjectFileChange::new(relative_path, kind))
    }

    pub const fn rescan_required() -> Self {
        Self::RescanRequired
    }

    pub fn affects_project_index(&self) -> bool {
        match self {
            Self::File(change) => change.affects_project_index(),
            Self::RescanRequired => true,
        }
    }
}

impl From<ProjectFileChange> for ProjectChange {
    fn from(change: ProjectFileChange) -> Self {
        Self::File(change)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectIndexInvalidation {
    project_instance_id: ProjectInstanceId,
}

impl ProjectIndexInvalidation {
    pub fn new(project_instance_id: ProjectInstanceId) -> Self {
        Self {
            project_instance_id,
        }
    }

    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn into_project_instance_id(self) -> ProjectInstanceId {
        self.project_instance_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_constructor_enforces_its_declared_invariant() {
        assert_eq!(
            ProjectRelativePath::try_new(""),
            Err(ProjectRelativePathError::Empty)
        );
        assert_eq!(
            ProjectRelativePath::try_new(std::env::current_dir().unwrap().join("metadata.yssbi")),
            Err(ProjectRelativePathError::Absolute)
        );
        assert_eq!(
            ProjectRelativePath::try_new(PathBuf::from("events").join("..").join("metadata.yssbi")),
            Err(ProjectRelativePathError::NonNormalComponent)
        );
    }

    #[test]
    fn file_changes_share_the_canonical_project_index_filter() {
        let relevant = ProjectChange::file(
            ProjectRelativePath::try_new("events/foo.yssbi-event").unwrap(),
            ProjectFileChangeKind::Modified,
        );
        let unrelated = ProjectChange::file(
            ProjectRelativePath::try_new("README.md").unwrap(),
            ProjectFileChangeKind::Modified,
        );

        assert!(relevant.affects_project_index());
        assert!(!unrelated.affects_project_index());
    }

    #[test]
    fn source_uncertainty_requests_a_rescan_without_faking_a_file_path() {
        let change = ProjectChange::rescan_required();

        assert_eq!(change, ProjectChange::RescanRequired);
        assert!(change.affects_project_index());
    }

    #[test]
    fn invalidation_preserves_the_strong_runtime_project_identity() {
        let project = ProjectInstanceId::from_existing("runtime-project".to_owned());
        let invalidation = ProjectIndexInvalidation::new(project.clone());

        assert_eq!(invalidation.project_instance_id(), &project);
        assert_eq!(invalidation.into_project_instance_id(), project);
    }
}
