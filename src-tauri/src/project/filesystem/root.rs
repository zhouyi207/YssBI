use crate::project::{PROJECT_METADATA_FILE, ProjectFilesystemError, project_root_from_path};
use std::cmp::Ordering;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use yss_project_identity::ProjectRootIdentity;

#[cfg(test)]
thread_local! {
    static NORMALIZED_ROOT_RECONSTRUCTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(windows)]
use super::windows_path_identity::WindowsPathIdentity;

#[cfg(windows)]
type NativePathIdentity = WindowsPathIdentity;
#[cfg(not(windows))]
type NativePathIdentity = PathBuf;

#[derive(Clone, Debug)]
pub struct ProjectRootBinding {
    caller_root: PathBuf,
    normalized: NormalizedProjectRoot,
    identity: Option<ProjectRootIdentity>,
}

impl ProjectRootBinding {
    pub fn for_destination(path: impl AsRef<Path>) -> Result<Self, ProjectFilesystemError> {
        let caller_root = validate_caller_root_components(path.as_ref(), false)?;
        let normalized = NormalizedProjectRoot::from_project_path(&caller_root)?;
        let identity = root_object_identity_if_existing(&caller_root)?;
        Ok(Self {
            caller_root,
            normalized,
            identity,
        })
    }

    pub fn for_existing(path: impl AsRef<Path>) -> Result<Self, ProjectFilesystemError> {
        let caller_root = validate_caller_root_components(path.as_ref(), true)?;
        let normalized = NormalizedProjectRoot::from_project_path(&caller_root)?;
        let identity = Some(root_object_identity(&caller_root)?);
        Ok(Self {
            caller_root,
            normalized,
            identity,
        })
    }

    pub fn normalized(&self) -> &NormalizedProjectRoot {
        &self.normalized
    }

    pub fn identity(&self) -> Option<&ProjectRootIdentity> {
        self.identity.as_ref()
    }

    pub fn bind_existing(&self) -> Result<Self, ProjectFilesystemError> {
        let rebound = Self::for_existing(&self.caller_root)?;
        if rebound.normalized != self.normalized {
            return Err(invalid_root(
                &self.caller_root,
                "project root changed while waiting",
            ));
        }
        Ok(rebound)
    }

    pub fn revalidate(&self) -> Result<(), ProjectFilesystemError> {
        let rebound = Self::for_existing(&self.caller_root)?;
        if rebound.normalized != self.normalized || rebound.identity != self.identity {
            return Err(invalid_root(
                &self.caller_root,
                "project root native identity changed",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct NormalizedProjectRoot {
    path: PathBuf,
    identity: NativePathIdentity,
}

impl NormalizedProjectRoot {
    pub fn from_project_path(path: impl AsRef<Path>) -> Result<Self, ProjectFilesystemError> {
        #[cfg(test)]
        NORMALIZED_ROOT_RECONSTRUCTIONS.set(NORMALIZED_ROOT_RECONSTRUCTIONS.get() + 1);
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

#[cfg(test)]
pub(crate) fn reset_normalized_root_reconstruction_count_for_test() {
    NORMALIZED_ROOT_RECONSTRUCTIONS.set(0);
}

#[cfg(test)]
pub(crate) fn normalized_root_reconstruction_count_for_test() -> usize {
    NORMALIZED_ROOT_RECONSTRUCTIONS.get()
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

fn validate_caller_root_components(
    path: &Path,
    require_existing: bool,
) -> Result<PathBuf, ProjectFilesystemError> {
    let original = path.to_path_buf();
    let trimmed = trim_path(path);
    if trimmed.as_os_str().is_empty() {
        return Err(invalid_root(original, "path is empty"));
    }
    let root = native_project_root_from_path(&trimmed);
    let absolute = if root.is_absolute() {
        root
    } else {
        std::env::current_dir()
            .map_err(|error| invalid_root(&original, error.to_string()))?
            .join(root)
    };
    let lexical = lexical_absolute_path(&absolute);
    let mut current = PathBuf::new();
    let mut missing = false;
    for component in lexical.components() {
        current.push(component.as_os_str());
        if missing || matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if super::transaction::metadata_is_redirect(&metadata) {
                    return Err(invalid_root(
                        &original,
                        format!(
                            "project root component '{}' is a redirect",
                            current.display()
                        ),
                    ));
                }
                if current != lexical && !metadata.is_dir() {
                    return Err(invalid_root(
                        &original,
                        format!(
                            "project root ancestor '{}' is not a directory",
                            current.display()
                        ),
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => missing = true,
            Err(error) => return Err(invalid_root(&original, error.to_string())),
        }
    }
    if require_existing && missing {
        return Err(invalid_root(&original, "project root does not exist"));
    }
    Ok(lexical)
}

fn lexical_absolute_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
        }
    }
    normalized
}

fn root_object_identity_if_existing(
    root: &Path,
) -> Result<Option<ProjectRootIdentity>, ProjectFilesystemError> {
    match std::fs::symlink_metadata(root) {
        Ok(_) => root_object_identity(root).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(invalid_root(root, error.to_string())),
    }
}

fn root_object_identity(root: &Path) -> Result<ProjectRootIdentity, ProjectFilesystemError> {
    let metadata =
        std::fs::symlink_metadata(root).map_err(|error| invalid_root(root, error.to_string()))?;
    if super::transaction::metadata_is_redirect(&metadata) || !metadata.is_dir() {
        return Err(invalid_root(root, "project root is not a real directory"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            GetFileInformationByHandle,
        };
        let directory = std::fs::OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(root)
            .map_err(|error| invalid_root(root, error.to_string()))?;
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        let success =
            unsafe { GetFileInformationByHandle(directory.as_raw_handle(), &mut information) };
        if success == 0 {
            return Err(invalid_root(
                root,
                std::io::Error::last_os_error().to_string(),
            ));
        }
        if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(invalid_root(root, "project root is a reparse point"));
        }
        let file = ((information.nFileIndexHigh as u64) << 32) | information.nFileIndexLow as u64;
        return Ok(ProjectRootIdentity::from_canonical(format!(
            "windows:{:08x}:{file:016x}",
            information.dwVolumeSerialNumber
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return Ok(ProjectRootIdentity::from_canonical(format!(
            "unix:{:016x}:{:016x}",
            metadata.dev(),
            metadata.ino()
        )));
    }
    #[allow(unreachable_code)]
    Err(invalid_root(
        root,
        "native project root identity is unsupported",
    ))
}

fn invalid_root(path: impl AsRef<Path>, message: impl Into<String>) -> ProjectFilesystemError {
    ProjectFilesystemError::InvalidRoot {
        path: path.as_ref().to_path_buf(),
        message: message.into(),
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
