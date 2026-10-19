//! Project registration workflows and path validation.
//!
//! Persistence records and the storage port live in
//! `yss-project-registry-contract`; concrete stores remain backend adapters.

use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

use yss_filesystem::RootBinding;
use yss_project_identity::ProjectRegistrationId;
use yss_project_layout::{PROJECT_METADATA_FILE, project_root_from_path};
use yss_project_model::normalize_project_name;
use yss_project_registry_contract::{
    ProjectRecord, ProjectRegistryStore, ProjectRegistryStoreError, ProjectRootIdentityState,
};

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
}

pub struct ProjectRegistry {
    store: Arc<dyn ProjectRegistryStore>,
}

impl ProjectRegistry {
    pub fn new(store: Arc<dyn ProjectRegistryStore>) -> Self {
        Self { store }
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectRecord>, ProjectRegistryError> {
        let mut records = self
            .store
            .load()
            .await
            .map_err(ProjectRegistryError::Store)?
            .into_vec();
        let timestamp = |record: &ProjectRecord| {
            record
                .last_opened_at
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
        };
        records.sort_by(|left, right| {
            timestamp(right).cmp(&timestamp(left)).then_with(|| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            })
        });
        Ok(records)
    }

    async fn fetch_by_root(
        &self,
        binding: &RootBinding,
        path: &str,
    ) -> Result<Option<ProjectRecord>, ProjectRegistryError> {
        let identity = binding
            .identity()
            .ok_or(ProjectRegistryError::RootIdentityMissing)?;
        let record = self
            .store
            .get_by_path(path)
            .await
            .map_err(ProjectRegistryError::Store)?;
        binding
            .revalidate()
            .map_err(|_| ProjectRegistryError::IdentityChanged)?;
        if record.as_ref().is_some_and(|record| {
            record.valid_root_identity().map(|value| value.as_str()) != Some(identity.as_str())
        }) {
            return Err(ProjectRegistryError::IdentityChanged);
        }
        Ok(record)
    }

    pub async fn register_project(
        &self,
        name: &str,
        path: &str,
    ) -> Result<ProjectRecord, ProjectRegistryError> {
        let binding = RootBinding::for_existing(project_root_from_path(path))
            .map_err(|_| ProjectRegistryError::InvalidPath)?;
        let root_identity = binding
            .identity()
            .map(|identity| {
                yss_project_identity::ProjectRootIdentity::from_canonical(
                    identity.as_str().to_owned(),
                )
            })
            .ok_or(ProjectRegistryError::RootIdentityMissing)?;
        let path = normalize_existing_path(path)?;

        if let Some(existing) = self.fetch_by_root(&binding, &path).await? {
            let updated = ProjectRecord {
                last_opened_at: Some(now_string()),
                path,
                ..existing
            };
            self.store
                .upsert(&updated)
                .await
                .map_err(ProjectRegistryError::Store)?;
            return Ok(updated);
        }

        let now = now_string();
        let record = ProjectRecord {
            id: ProjectRegistrationId::generate(),
            name: normalize_project_name(name),
            path,
            created_at: now.clone(),
            last_opened_at: Some(now),
            root_identity,
            root_identity_state: ProjectRootIdentityState::Valid,
        };
        self.store
            .upsert(&record)
            .await
            .map_err(ProjectRegistryError::Store)?;
        Ok(record)
    }
}

fn now_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    seconds.to_string()
}

pub fn default_project_parent_directory() -> Result<String, std::env::VarError> {
    #[cfg(windows)]
    let variable = "USERPROFILE";
    #[cfg(not(windows))]
    let variable = "HOME";
    let parent = PathBuf::from(std::env::var(variable)?);
    let documents = parent.join("Documents");
    let directory = if documents.is_dir() {
        &documents
    } else {
        &parent
    };
    Ok(dunce::simplified(directory).to_string_lossy().into_owned())
}

pub fn normalize_existing_path(path: &str) -> Result<String, ProjectRegistryError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(ProjectRegistryError::InvalidPath);
    }
    let input = PathBuf::from(path);
    let pb = if input.is_dir() {
        input.join(PROJECT_METADATA_FILE)
    } else {
        input
    };
    let metadata = std::fs::symlink_metadata(&pb).map_err(|_| ProjectRegistryError::InvalidPath)?;
    if !metadata.is_file() || yss_filesystem::metadata_is_redirect(&metadata) {
        return Err(ProjectRegistryError::InvalidPath);
    }
    let file_name = pb
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ProjectRegistryError::InvalidPath)?;
    if !file_name.eq_ignore_ascii_case(PROJECT_METADATA_FILE) {
        return Err(ProjectRegistryError::InvalidPath);
    }
    dunce::canonicalize(&pb)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|_| ProjectRegistryError::InvalidPath)
}

#[cfg(test)]
mod tests {
    use super::{ProjectRegistry, ProjectRegistryError, normalize_existing_path};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use yss_project_identity::{ProjectRegistrationId, ProjectRootIdentity};
    use yss_project_layout::PROJECT_METADATA_FILE;
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
        fn get_by_path(
            &self,
            path: &str,
        ) -> ProjectRegistryStoreFuture<'_, Result<Option<ProjectRecord>, ProjectRegistryStoreError>>
        {
            let path = path.to_owned();
            Box::pin(async move {
                Ok(self
                    .records
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .iter()
                    .find(|record| record.path == path)
                    .cloned())
            })
        }

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

    fn record(id: &str, name: &str, last_opened_at: Option<&str>) -> ProjectRecord {
        ProjectRecord {
            id: ProjectRegistrationId::from_existing(id.into()),
            name: name.into(),
            path: format!("/projects/{id}/{PROJECT_METADATA_FILE}"),
            created_at: "1".into(),
            last_opened_at: last_opened_at.map(Into::into),
            root_identity: ProjectRootIdentity::from_canonical(format!("root-{id}")),
            root_identity_state: ProjectRootIdentityState::Valid,
        }
    }

    fn registry(records: Vec<ProjectRecord>) -> ProjectRegistry {
        ProjectRegistry::new(Arc::new(MemoryProjectRegistryStore::with_records(records)))
    }

    #[tokio::test]
    async fn list_orders_recent_projects_then_names() {
        let registry = registry(vec![
            record("never", "Zulu", None),
            record("old-b", "bravo", Some("99")),
            record("old-a", "Alpha", Some("99")),
            record("new", "Charlie", Some("100")),
        ]);

        let records = registry.list_projects().await.expect("list projects");
        let ids = records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, ["new", "old-a", "old-b", "never"]);
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
            .register_project(
                "Ignored replacement",
                std::fs::canonicalize(root.join(PROJECT_METADATA_FILE))
                    .unwrap()
                    .to_str()
                    .unwrap(),
            )
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
    async fn registering_long_paths_preserves_a_reopenable_path() {
        let directory = TestDirectory::new("long-path");
        let mut root = std::fs::canonicalize(directory.path()).unwrap();
        for index in 0..5 {
            root.push(format!("{index}-{}", "segment".repeat(9)));
        }
        std::fs::create_dir_all(&root).unwrap();
        let metadata = root.join(PROJECT_METADATA_FILE);
        std::fs::write(&metadata, b"{}").unwrap();
        let registry = registry(Vec::new());
        let created = registry
            .register_project("Long", root.to_str().unwrap())
            .await
            .unwrap();

        assert_eq!(
            created.path,
            std::fs::canonicalize(&metadata).unwrap().to_string_lossy()
        );
        assert_eq!(std::fs::read(&created.path).unwrap(), b"{}");
        let reopened = registry
            .register_project("Ignored", &created.path)
            .await
            .unwrap();
        assert_eq!(created.id, reopened.id);
        assert_eq!(created.path, reopened.path);
        assert_eq!(registry.list_projects().await.unwrap().len(), 1);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn registration_rejects_redirected_metadata_without_creating_a_record() {
        let directory = TestDirectory::new("redirected-metadata");
        let root = directory.child("project");
        let outside = directory.child("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join(PROJECT_METADATA_FILE), "{}").unwrap();
        std::os::unix::fs::symlink(
            outside.join(PROJECT_METADATA_FILE),
            root.join(PROJECT_METADATA_FILE),
        )
        .unwrap();
        let registry = registry(Vec::new());
        assert!(matches!(
            registry
                .register_project("Redirected", root.to_str().unwrap())
                .await,
            Err(ProjectRegistryError::InvalidPath)
        ));
        assert!(registry.list_projects().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn registering_rejects_a_replaced_root() {
        let directory = TestDirectory::new("replaced-root");
        let root = directory.child("project");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join(PROJECT_METADATA_FILE), "{}").unwrap();
        let registry = registry(Vec::new());
        let original = registry
            .register_project("Original", root.to_str().unwrap())
            .await
            .unwrap();

        std::fs::rename(&root, directory.child("original-root")).unwrap();
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join(PROJECT_METADATA_FILE), "{}").unwrap();
        let binding = yss_filesystem::RootBinding::for_existing(
            yss_project_layout::project_root_from_path(&root),
        )
        .unwrap();
        assert_ne!(
            binding.identity().map(|identity| identity.as_str()),
            Some(original.root_identity.as_str())
        );

        assert!(matches!(
            registry
                .register_project("Replacement", root.to_str().unwrap())
                .await,
            Err(ProjectRegistryError::IdentityChanged)
        ));
        assert_eq!(registry.list_projects().await.unwrap(), [original]);
    }
}
