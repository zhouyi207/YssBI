use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

use super::{
    NormalizedProjectRoot, ProjectDiscoveryError, ProjectRootBinding, format_path_for_user,
    format_path_for_user_path,
};

use yss_project_identity::ProjectRegistrationId;
use yss_project_progress::{
    ProjectCleanupProgress, ProjectProgress, ProjectProgressSink, ProjectScanProgress,
    ProjectTaskCancellation,
};
use yss_project_registry_contract::{
    ProjectRecord, ProjectRegistryStore, ProjectRegistryStoreError, ProjectRootIdentityState,
};

pub const PROJECT_METADATA_FILE: &str = "metadata.yssbi";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupInvalidProjectsResult {
    pub removed: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectPathValidationError {
    Empty,
    NotDirectory,
    AlreadyContainsProject,
    NotEmpty,
    InvalidParent,
    ParentUnavailable,
}

impl ProjectPathValidationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Empty => "project_path_empty",
            Self::NotDirectory => "project_path_not_directory",
            Self::AlreadyContainsProject => "project_path_already_contains_project",
            Self::NotEmpty => "project_path_not_empty",
            Self::InvalidParent => "project_parent_path_invalid",
            Self::ParentUnavailable => "project_parent_unavailable",
        }
    }
}

#[derive(Debug, Error)]
pub enum ProjectRegistryError {
    #[error("project registry storage failed")]
    Store(#[source] ProjectRegistryStoreError),
    #[error("project path is invalid")]
    InvalidPath,
    #[error("project root identity is missing")]
    RootIdentityMissing,
    #[error("registered project root identity changed")]
    IdentityChanged,
    #[error("project registry record is unavailable")]
    NotFound,
    #[error("project registry operation was cancelled")]
    Cancelled,
    #[error("project scan failed")]
    ScanFailed,
}

pub struct ProjectRegistry {
    store: Arc<dyn ProjectRegistryStore>,
    path: PathBuf,
    #[cfg(test)]
    fail_remove: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ProjectRegistry {
    pub fn new(store: Arc<dyn ProjectRegistryStore>, path: PathBuf) -> Self {
        Self {
            store,
            path,
            #[cfg(test)]
            fail_remove: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    pub async fn init(app_dir: PathBuf) -> Result<Self, ProjectRegistryError> {
        let store =
            crate::backend_adapters::project_registry_sqlite::SqliteProjectRegistryStore::connect(
                app_dir,
            )
            .await
            .map_err(|_| ProjectRegistryError::Store(ProjectRegistryStoreError::StorageFailed))?;
        let path = store.path().to_path_buf();
        Ok(Self::new(Arc::new(store), path))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(test)]
    pub(crate) async fn fail_project_remove_for_test(&self) {
        self.fail_remove
            .store(true, std::sync::atomic::Ordering::Release);
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectRecord>, ProjectRegistryError> {
        let mut records = self
            .store
            .load()
            .await
            .map_err(ProjectRegistryError::Store)?
            .into_vec();
        records.sort_by(|left, right| {
            right
                .is_favorite
                .cmp(&left.is_favorite)
                .then_with(|| right.last_opened_at.cmp(&left.last_opened_at))
                .then_with(|| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                })
        });
        Ok(records)
    }

    pub async fn fetch_by_id(
        &self,
        id: &str,
    ) -> Result<Option<ProjectRecord>, ProjectRegistryError> {
        Ok(self
            .list_projects()
            .await?
            .into_iter()
            .find(|record| record.id.as_str() == id))
    }

    async fn fetch_by_path(
        &self,
        path: &str,
    ) -> Result<Option<ProjectRecord>, ProjectRegistryError> {
        let normalized = normalize_existing_path(path).ok();
        let records = self.list_projects().await?;
        Ok(records.into_iter().find(|record| {
            normalized.as_deref().is_some_and(|canonical| {
                NormalizedProjectRoot::from_project_path(&record.path).ok()
                    == NormalizedProjectRoot::from_project_path(canonical).ok()
            }) || record.path == path
        }))
    }

    pub async fn register_project(
        &self,
        name: &str,
        path: &str,
    ) -> Result<ProjectRecord, ProjectRegistryError> {
        let binding = ProjectRootBinding::for_existing(path)
            .map_err(|_| ProjectRegistryError::InvalidPath)?;
        let root_identity = binding
            .identity()
            .cloned()
            .ok_or(ProjectRegistryError::RootIdentityMissing)?;
        let path = normalize_existing_path(path).map_err(|_| ProjectRegistryError::InvalidPath)?;

        if let Some(existing) = self.fetch_by_path(&path).await? {
            if existing.deletion_identity() != Some(&root_identity) {
                return Err(ProjectRegistryError::IdentityChanged);
            }
            let updated = ProjectRecord {
                last_opened_at: Some(now_string()),
                ..existing
            };
            self.store
                .upsert(&updated)
                .await
                .map_err(ProjectRegistryError::Store)?;
            return Ok(updated);
        }

        let record = ProjectRecord {
            id: ProjectRegistrationId::generate(),
            name: normalize_project_name(name),
            path,
            created_at: now_string(),
            last_opened_at: Some(now_string()),
            is_favorite: false,
            root_identity,
            root_identity_state: ProjectRootIdentityState::Valid,
        };
        self.store
            .upsert(&record)
            .await
            .map_err(ProjectRegistryError::Store)?;
        Ok(record)
    }

    pub async fn remove_project(&self, id: &str) -> Result<(), ProjectRegistryError> {
        #[cfg(test)]
        if self.fail_remove.load(std::sync::atomic::Ordering::Acquire) {
            return Err(ProjectRegistryError::Store(
                ProjectRegistryStoreError::StorageFailed,
            ));
        }
        let registration = ProjectRegistrationId::from_existing(id.to_owned());
        self.store
            .remove(&registration)
            .await
            .map_err(|error| match error {
                ProjectRegistryStoreError::Unavailable => ProjectRegistryError::NotFound,
                other => ProjectRegistryError::Store(other),
            })
    }

    pub async fn toggle_favorite(&self, id: &str) -> Result<bool, ProjectRegistryError> {
        let record = self
            .fetch_by_id(id)
            .await?
            .ok_or(ProjectRegistryError::NotFound)?;
        let favorite = !record.is_favorite;
        self.store
            .upsert(&ProjectRecord {
                is_favorite: favorite,
                ..record
            })
            .await
            .map_err(ProjectRegistryError::Store)?;
        Ok(favorite)
    }

    pub async fn cleanup_invalid_projects(
        &self,
        progress: Option<&dyn ProjectProgressSink>,
        cancellation: ProjectTaskCancellation,
    ) -> Result<CleanupInvalidProjectsResult, ProjectRegistryError> {
        if cancellation.is_cancelled() {
            return Err(ProjectRegistryError::Cancelled);
        }
        let projects = self.list_projects().await?;
        let total = projects.len();
        let mut removed = 0usize;
        for (index, project) in projects.iter().enumerate() {
            if cancellation.is_cancelled() {
                return Err(ProjectRegistryError::Cancelled);
            }
            if let Some(sink) = progress {
                sink.publish(ProjectProgress::Cleanup(ProjectCleanupProgress::Checking {
                    current: index + 1,
                    total,
                }));
            }
            let current_identity = (project.root_identity_state == ProjectRootIdentityState::Valid)
                .then(|| ProjectRootBinding::for_existing(&project.path).ok())
                .flatten()
                .and_then(|binding| binding.identity().cloned());
            if project.root_identity_state == ProjectRootIdentityState::Valid
                && current_identity.as_ref() == Some(&project.root_identity)
            {
                continue;
            }
            self.remove_project(project.id.as_str()).await?;
            removed += 1;
            if let Some(sink) = progress {
                sink.publish(ProjectProgress::Cleanup(ProjectCleanupProgress::Removing {
                    removed,
                    total,
                }));
            }
        }
        Ok(CleanupInvalidProjectsResult { removed })
    }

    pub async fn scan_directory(
        &self,
        directory: &str,
        progress: Option<&dyn ProjectProgressSink>,
        cancellation: ProjectTaskCancellation,
    ) -> Result<crate::project::ScanProjectsResult, ProjectRegistryError> {
        if cancellation.is_cancelled() {
            return Err(ProjectRegistryError::Cancelled);
        }
        if let Some(sink) = progress {
            sink.publish(ProjectProgress::Scan(ProjectScanProgress::Scanning));
        }
        let root = PathBuf::from(directory.trim());
        let metadata_files = crate::project::discover_project_metadata_files(&root, &cancellation)
            .map_err(|error| match error {
                ProjectDiscoveryError::Cancelled => ProjectRegistryError::Cancelled,
                ProjectDiscoveryError::InvalidRoot | ProjectDiscoveryError::Io(_) => {
                    ProjectRegistryError::ScanFailed
                }
            })?;
        if let Some(sink) = progress {
            sink.publish(ProjectProgress::Scan(ProjectScanProgress::Discovered {
                count: metadata_files.len(),
            }));
        }
        let mut newly_registered = 0;
        let mut projects = Vec::with_capacity(metadata_files.len());
        for (index, metadata_path) in metadata_files.iter().enumerate() {
            if cancellation.is_cancelled() {
                return Err(ProjectRegistryError::Cancelled);
            }
            if let Some(sink) = progress {
                sink.publish(ProjectProgress::Scan(ProjectScanProgress::Registering {
                    current: index + 1,
                    total: metadata_files.len(),
                }));
            }
            let path = metadata_path.to_string_lossy().into_owned();
            let normalized =
                normalize_existing_path(&path).map_err(|_| ProjectRegistryError::ScanFailed)?;
            let name = crate::project::project_name_from_metadata_path(metadata_path);
            let existing = self.fetch_by_path(&normalized).await?;
            let (record, is_new) = match existing {
                Some(record) => (record, false),
                None => (self.register_project(&name, &normalized).await?, true),
            };
            newly_registered += usize::from(is_new);
            projects.push(record);
        }
        Ok(crate::project::ScanProjectsResult {
            discovered: metadata_files.len(),
            newly_registered,
            projects,
        })
    }
}

fn now_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    seconds.to_string()
}

pub fn normalize_project_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "未命名项目".into()
    } else {
        name.into()
    }
}

pub fn default_project_parent_directory() -> Result<String, String> {
    #[cfg(windows)]
    {
        let userprofile =
            std::env::var("USERPROFILE").map_err(|_| "无法读取用户目录".to_string())?;
        let docs = PathBuf::from(&userprofile).join("Documents");
        if docs.is_dir() {
            Ok(format_path_for_user_path(&docs))
        } else {
            Ok(format_path_for_user(&userprofile))
        }
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").map_err(|_| "无法读取 HOME".to_string())?;
        let docs = PathBuf::from(&home).join("Documents");
        if docs.is_dir() {
            Ok(format_path_for_user_path(&docs))
        } else {
            Ok(format_path_for_user(&home))
        }
    }
}

pub fn validate_new_project_path(path: &str) -> Result<(), ProjectPathValidationError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(ProjectPathValidationError::Empty);
    }
    let pb = PathBuf::from(path);
    if pb.exists() {
        if pb.is_file() {
            return Err(ProjectPathValidationError::NotDirectory);
        }
        if pb.join(PROJECT_METADATA_FILE).exists() {
            return Err(ProjectPathValidationError::AlreadyContainsProject);
        }
        if directory_has_entries(&pb) {
            return Err(ProjectPathValidationError::NotEmpty);
        }
        return Ok(());
    }
    let Some(parent) = pb.parent().filter(|parent| !parent.as_os_str().is_empty()) else {
        return Err(ProjectPathValidationError::InvalidParent);
    };
    if !parent.exists() || !parent.is_dir() {
        return Err(ProjectPathValidationError::ParentUnavailable);
    }
    Ok(())
}

fn directory_has_entries(path: &Path) -> bool {
    std::fs::read_dir(path).map_or(true, |mut entries| entries.next().is_some())
}

pub fn is_registered_project_valid(path: &str) -> bool {
    normalize_existing_path(path).is_ok()
}

pub fn normalize_existing_path(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("路径不能为空".into());
    }
    let input = PathBuf::from(path);
    let pb = if input.is_dir() {
        input.join(PROJECT_METADATA_FILE)
    } else {
        input
    };
    if !pb.exists() {
        return Err("项目文件不存在".into());
    }
    if !pb.is_file() {
        return Err("项目路径必须是文件".into());
    }
    let file_name = pb
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "无效的项目文件路径".to_string())?;
    if !file_name.eq_ignore_ascii_case(PROJECT_METADATA_FILE) {
        return Err(format!("项目文件必须是 {PROJECT_METADATA_FILE}"));
    }
    std::fs::canonicalize(&pb)
        .map(|path| format_path_for_user_path(&path))
        .map_err(|error| format!("无法解析项目路径: {error}"))
}
