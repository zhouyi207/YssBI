use crate::project::{PROJECT_METADATA_FILE, ProjectFilesystemError, project_root_from_path};
use std::cmp::Ordering;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};

#[cfg(windows)]
use super::windows_path_identity::WindowsPathIdentity;

#[cfg(windows)]
type NativePathIdentity = WindowsPathIdentity;
#[cfg(not(windows))]
type NativePathIdentity = PathBuf;

#[derive(Clone, Debug)]
pub struct NormalizedProjectRoot {
    path: PathBuf,
    identity: NativePathIdentity,
}

impl NormalizedProjectRoot {
    pub fn from_project_path(path: impl AsRef<Path>) -> Result<Self, ProjectFilesystemError> {
        let original = path.as_ref().to_path_buf();
        let trimmed = trim_path(&original);
        if trimmed.as_os_str().is_empty() {
            return Err(ProjectFilesystemError::InvalidRoot {
                path: original,
                message: "path is empty".into(),
            });
        }

        let project_root = native_project_root_from_path(&trimmed);
        let absolute = if project_root.is_absolute() {
            project_root
        } else {
            std::env::current_dir()
                .map_err(|error| ProjectFilesystemError::InvalidRoot {
                    path: original.clone(),
                    message: format!("failed to resolve current directory: {error}"),
                })?
                .join(project_root)
        };
        let path = normalize_from_existing_ancestor(&absolute).map_err(|error| {
            ProjectFilesystemError::InvalidRoot {
                path: original.clone(),
                message: error.to_string(),
            }
        })?;
        let identity =
            native_path_identity(&path).map_err(|error| ProjectFilesystemError::InvalidRoot {
                path: original,
                message: format!("failed to build filesystem identity: {error}"),
            })?;

        Ok(Self { path, identity })
    }

    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

impl PartialEq for NormalizedProjectRoot {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl Eq for NormalizedProjectRoot {}

impl Hash for NormalizedProjectRoot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
    }
}

impl PartialOrd for NormalizedProjectRoot {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NormalizedProjectRoot {
    fn cmp(&self, other: &Self) -> Ordering {
        self.identity.cmp(&other.identity)
    }
}

fn trim_path(path: &Path) -> PathBuf {
    path.to_str()
        .map(|text| PathBuf::from(text.trim()))
        .unwrap_or_else(|| path.to_path_buf())
}

fn native_project_root_from_path(path: &Path) -> PathBuf {
    if let Some(path) = path.to_str() {
        return project_root_from_path(path);
    }
    let is_metadata_file = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(PROJECT_METADATA_FILE));
    if path.is_file() || is_metadata_file {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

fn normalize_from_existing_ancestor(path: &Path) -> std::io::Result<PathBuf> {
    let mut existing = PathBuf::new();
    let mut missing = Vec::<OsString>::new();
    let mut found_missing_component = false;

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => existing.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir if found_missing_component => {
                missing.pop();
            }
            Component::ParentDir => {
                existing.pop();
            }
            Component::Normal(name) if !found_missing_component => {
                let candidate = existing.join(name);
                if candidate.exists() {
                    existing = candidate;
                } else {
                    found_missing_component = true;
                    missing.push(name.to_os_string());
                }
            }
            Component::Normal(name) => missing.push(name.to_os_string()),
        }
    }

    let mut normalized = std::fs::canonicalize(existing)?;
    for component in missing {
        normalized.push(component);
    }
    Ok(normalized)
}

#[cfg(windows)]
fn native_path_identity(path: &Path) -> std::io::Result<NativePathIdentity> {
    WindowsPathIdentity::from_os_str(path.as_os_str())
}

#[cfg(not(windows))]
fn native_path_identity(path: &Path) -> std::io::Result<NativePathIdentity> {
    Ok(path.to_path_buf())
}
