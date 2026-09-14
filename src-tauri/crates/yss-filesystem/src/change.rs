//! Safe relative paths and filesystem change facts.

#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelativePathError {
    Empty,
    Absolute,
    NonNormalComponent,
}

impl fmt::Display for RelativePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "root-relative path is empty",
            Self::Absolute => "root-relative path must not be absolute",
            Self::NonNormalComponent => {
                "root-relative path contains a current, parent, root, or prefix component"
            }
        })
    }
}

impl Error for RelativePathError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RelativePath(PathBuf);

impl RelativePath {
    pub fn try_new(path: impl Into<PathBuf>) -> Result<Self, RelativePathError> {
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(RelativePathError::Empty);
        }
        if path.is_absolute() {
            return Err(RelativePathError::Absolute);
        }
        if path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(RelativePathError::NonNormalComponent);
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
pub enum FileChangeKind {
    Created,
    Modified,
    Removed,
    Renamed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChange {
    relative_path: RelativePath,
    kind: FileChangeKind,
}

impl FileChange {
    pub fn new(relative_path: RelativePath, kind: FileChangeKind) -> Self {
        Self {
            relative_path,
            kind,
        }
    }

    pub fn relative_path(&self) -> &RelativePath {
        &self.relative_path
    }

    pub fn kind(&self) -> FileChangeKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilesystemChange {
    File(FileChange),
    RescanRequired,
}

impl FilesystemChange {
    pub fn file(relative_path: RelativePath, kind: FileChangeKind) -> Self {
        Self::File(FileChange::new(relative_path, kind))
    }

    pub const fn rescan_required() -> Self {
        Self::RescanRequired
    }
}

impl From<FileChange> for FilesystemChange {
    fn from(change: FileChange) -> Self {
        Self::File(change)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_constructor_enforces_its_declared_invariant() {
        assert_eq!(RelativePath::try_new(""), Err(RelativePathError::Empty));
        assert_eq!(
            RelativePath::try_new(std::env::current_dir().unwrap().join("metadata.yssbi")),
            Err(RelativePathError::Absolute)
        );
        assert_eq!(
            RelativePath::try_new(PathBuf::from("events").join("..").join("metadata.yssbi")),
            Err(RelativePathError::NonNormalComponent)
        );
    }
}
