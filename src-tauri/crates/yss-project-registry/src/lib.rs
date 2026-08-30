//! Project registration workflows and path validation.
//!
//! Persistence records and the storage port live in
//! `yss-project-registry-contract`; concrete stores remain backend adapters.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

use yss_path_display::{format_path_for_user, format_path_for_user_path};
use yss_project_discovery::{
    ProjectDiscoveryError, discover_project_metadata_files, normalize_project_name,
    project_name_from_metadata_path,
};
use yss_project_filesystem::{NormalizedProjectRoot, ProjectRootBinding};
use yss_project_identity::ProjectRegistrationId;
use yss_project_layout::PROJECT_METADATA_FILE;
use yss_project_progress::{
    ProjectCleanupProgress, ProjectProgress, ProjectProgressSink, ProjectScanProgress,
    ProjectTaskCancellation,
};
use yss_project_registry_contract::{
    ProjectRecord, ProjectRegistryStore, ProjectRegistryStoreError, ProjectRootIdentityState,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupInvalidProjectsResult {
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProjectsResult {
    pub discovered: usize,
    pub newly_registered: usize,
    pub projects: Vec<ProjectRecord>,
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
}

impl ProjectRegistry {
    pub fn new(store: Arc<dyn ProjectRegistryStore>, path: PathBuf) -> Self {
        Self { store, path }
    }

    pub fn path(&self) -> &Path {
        &self.path
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
    ) -> Result<ScanProjectsResult, ProjectRegistryError> {
        if cancellation.is_cancelled() {
            return Err(ProjectRegistryError::Cancelled);
        }
        if let Some(sink) = progress {
            sink.publish(ProjectProgress::Scan(ProjectScanProgress::Scanning));
        }
        let root = PathBuf::from(directory.trim());
        let metadata_files =
            discover_project_metadata_files(&root, &cancellation).map_err(|error| match error {
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
            let name = project_name_from_metadata_path(metadata_path);
            let existing = self.fetch_by_path(&normalized).await?;
            let (record, is_new) = match existing {
                Some(record) => (record, false),
                None => (self.register_project(&name, &normalized).await?, true),
            };
            newly_registered += usize::from(is_new);
            projects.push(record);
        }
        Ok(ScanProjectsResult {
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

#[cfg(test)]
mod tests {
    use super::{
        ProjectPathValidationError, ProjectRegistry, ProjectRegistryError, normalize_existing_path,
        validate_new_project_path,
    };
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use yss_project_identity::{ProjectRegistrationId, ProjectRootIdentity};
    use yss_project_layout::PROJECT_METADATA_FILE;
    use yss_project_progress::ProjectTaskCancellationRegistry;
    use yss_project_registry_contract::{
        ProjectRecord, ProjectRegistryStore, ProjectRegistryStoreError, ProjectRegistryStoreFuture,
        ProjectRootIdentityState,
    };

    #[derive(Default)]
    struct MemoryProjectRegistryStore {
        records: Mutex<Vec<ProjectRecord>>,
    }

    impl MemoryProjectRegistryStore {
        fn with_records(records: Vec<ProjectRecord>) -> Self {
            Self {
                records: Mutex::new(records),
            }
        }
    }

    impl ProjectRegistryStore for MemoryProjectRegistryStore {
        fn load(
            &self,
        ) -> ProjectRegistryStoreFuture<'_, Result<Box<[ProjectRecord]>, ProjectRegistryStoreError>>
        {
            Box::pin(async move {
                Ok(self
                    .records
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone()
                    .into_boxed_slice())
            })
        }

        fn upsert(
            &self,
            record: &ProjectRecord,
        ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>> {
            let record = record.clone();
            Box::pin(async move {
                let mut records = self
                    .records
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(existing) = records.iter_mut().find(|item| item.id == record.id) {
                    *existing = record;
                } else {
                    records.push(record);
                }
                Ok(())
            })
        }

        fn remove(
            &self,
            registration: &ProjectRegistrationId,
        ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>> {
            let registration = registration.clone();
            Box::pin(async move {
                let mut records = self
                    .records
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let Some(index) = records.iter().position(|item| item.id == registration) else {
                    return Err(ProjectRegistryStoreError::Unavailable);
                };
                records.remove(index);
                Ok(())
            })
        }
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "yss-project-registry-{label}-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn child(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn record(id: &str, name: &str, favorite: bool, last_opened_at: Option<&str>) -> ProjectRecord {
        ProjectRecord {
            id: ProjectRegistrationId::from_existing(id.into()),
            name: name.into(),
            path: format!("/projects/{id}/{PROJECT_METADATA_FILE}"),
            created_at: "1".into(),
            last_opened_at: last_opened_at.map(Into::into),
            is_favorite: favorite,
            root_identity: ProjectRootIdentity::from_canonical(format!("root-{id}")),
            root_identity_state: ProjectRootIdentityState::Valid,
        }
    }

    fn registry(records: Vec<ProjectRecord>) -> ProjectRegistry {
        ProjectRegistry::new(
            Arc::new(MemoryProjectRegistryStore::with_records(records)),
            PathBuf::from("memory://project-registry"),
        )
    }

    #[tokio::test]
    async fn list_orders_favorites_then_recent_projects_then_names() {
        let registry = registry(vec![
            record("plain", "Zulu", false, Some("99")),
            record("old", "bravo", true, Some("10")),
            record("new", "Alpha", true, Some("20")),
        ]);

        let records = registry.list_projects().await.expect("list projects");
        let ids = records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, ["new", "old", "plain"]);
    }

    #[tokio::test]
    async fn toggle_and_remove_use_the_canonical_store_record() {
        let registry = registry(vec![record("one", "One", false, None)]);

        assert!(registry.toggle_favorite("one").await.expect("toggle"));
        assert!(
            registry
                .fetch_by_id("one")
                .await
                .expect("fetch")
                .expect("record")
                .is_favorite
        );
        registry.remove_project("one").await.expect("remove");
        assert!(registry.fetch_by_id("one").await.expect("fetch").is_none());
        assert!(matches!(
            registry.remove_project("missing").await,
            Err(ProjectRegistryError::NotFound)
        ));
    }

    #[tokio::test]
    async fn registering_an_existing_project_is_idempotent_by_root_identity() {
        let directory = TestDirectory::new("register");
        let root = directory.child("project");
        std::fs::create_dir_all(&root).expect("create project root");
        std::fs::write(root.join(PROJECT_METADATA_FILE), b"{}").expect("write metadata");
        let registry = registry(Vec::new());

        let created = registry
            .register_project("  Example  ", root.to_string_lossy().as_ref())
            .await
            .expect("register project");
        let reopened = registry
            .register_project("Ignored replacement", root.to_string_lossy().as_ref())
            .await
            .expect("re-register project");

        assert_eq!(created.id, reopened.id);
        assert_eq!(reopened.name, "Example");
        assert_eq!(
            reopened.path,
            normalize_existing_path(root.to_string_lossy().as_ref()).expect("normalized path")
        );
        assert_eq!(registry.list_projects().await.expect("list").len(), 1);
    }

    #[tokio::test]
    async fn cancelled_scan_preserves_the_typed_cancelled_outcome() {
        let registry = registry(Vec::new());
        let cancellations = ProjectTaskCancellationRegistry::new();
        let cancellation = cancellations.begin();
        cancellations.cancel_active();

        assert!(matches!(
            registry.scan_directory("ignored", None, cancellation).await,
            Err(ProjectRegistryError::Cancelled)
        ));
    }

    #[test]
    fn new_project_path_validation_distinguishes_conflict_kinds() {
        let directory = TestDirectory::new("path-validation");
        let target = directory.child("target");
        std::fs::create_dir_all(&target).expect("create empty target");

        assert_eq!(
            validate_new_project_path(""),
            Err(ProjectPathValidationError::Empty)
        );
        assert_eq!(
            validate_new_project_path(target.to_string_lossy().as_ref()),
            Ok(())
        );

        std::fs::write(target.join("occupied.txt"), b"occupied").expect("occupy target");
        assert_eq!(
            validate_new_project_path(target.to_string_lossy().as_ref()),
            Err(ProjectPathValidationError::NotEmpty)
        );
        std::fs::remove_file(target.join("occupied.txt")).expect("clear target");
        std::fs::write(target.join(PROJECT_METADATA_FILE), b"{}").expect("write metadata");
        assert_eq!(
            validate_new_project_path(target.to_string_lossy().as_ref()),
            Err(ProjectPathValidationError::AlreadyContainsProject)
        );

        let future = directory.child("future");
        assert_eq!(
            validate_new_project_path(future.to_string_lossy().as_ref()),
            Ok(())
        );
        let unavailable = directory.child("missing-parent").join("future");
        assert_eq!(
            validate_new_project_path(unavailable.to_string_lossy().as_ref()),
            Err(ProjectPathValidationError::ParentUnavailable)
        );
        assert!(directory.path().is_dir());
    }
}
