use super::SqliteProjectRegistryStore;
use std::path::{Path, PathBuf};
use yss_project_identity::{ProjectRegistrationId, ProjectRootIdentity};
use yss_project_registry_contract::{
    ProjectRecord, ProjectRegistryStore, ProjectRegistryStoreError, ProjectRootIdentityState,
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "yss-project-registry-sqlite-{label}-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn record(id: &str, path: &str) -> ProjectRecord {
    ProjectRecord {
        id: ProjectRegistrationId::from_existing(id.into()),
        name: format!("Project {id}"),
        path: path.into(),
        created_at: "17".into(),
        last_opened_at: Some("19".into()),
        is_favorite: false,
        root_identity: ProjectRootIdentity::from_canonical(format!("root-{id}")),
        root_identity_state: ProjectRootIdentityState::Valid,
    }
}

#[tokio::test]
async fn sqlite_store_round_trips_updates_and_removes_canonical_records() {
    let directory = TestDirectory::new("round-trip");
    let store = SqliteProjectRegistryStore::connect(directory.path().to_path_buf())
        .await
        .expect("connect store");
    assert_eq!(
        store.path(),
        directory.path().join("db").join("projects.sqlite")
    );

    let mut expected = record("registration-1", "C:/projects/one/metadata.yssbi");
    store.upsert(&expected).await.expect("insert record");
    assert_eq!(
        store.load().await.expect("load records").as_ref(),
        &[expected.clone()]
    );

    expected.name = "Renamed".into();
    expected.is_favorite = true;
    store.upsert(&expected).await.expect("update record");
    assert_eq!(
        store.load().await.expect("load update").as_ref(),
        &[expected.clone()]
    );

    store.remove(&expected.id).await.expect("remove record");
    assert!(store.load().await.expect("load empty").is_empty());
    assert_eq!(
        store.remove(&expected.id).await,
        Err(ProjectRegistryStoreError::Unavailable)
    );
    store.pool.close().await;
}

#[tokio::test]
async fn corrupt_persisted_discriminants_fail_closed() {
    let directory = TestDirectory::new("invalid-discriminants");
    let store = SqliteProjectRegistryStore::connect(directory.path().to_path_buf())
        .await
        .expect("connect store");
    let expected = record("registration-1", "C:/projects/one/metadata.yssbi");
    store.upsert(&expected).await.expect("insert record");

    sqlx::query("UPDATE projects SET root_identity_state = 'legacy'")
        .execute(&store.pool)
        .await
        .expect("corrupt identity state");
    assert_eq!(
        store.load().await,
        Err(ProjectRegistryStoreError::StorageFailed)
    );

    sqlx::query("UPDATE projects SET root_identity_state = 'valid', is_favorite = 2")
        .execute(&store.pool)
        .await
        .expect("corrupt favorite state");
    assert_eq!(
        store.load().await,
        Err(ProjectRegistryStoreError::StorageFailed)
    );
    store.pool.close().await;
}

#[tokio::test]
async fn duplicate_project_paths_map_to_the_storage_failure_contract() {
    let directory = TestDirectory::new("path-conflict");
    let store = SqliteProjectRegistryStore::connect(directory.path().to_path_buf())
        .await
        .expect("connect store");
    let first = record("registration-1", "C:/projects/shared/metadata.yssbi");
    let second = record("registration-2", "C:/projects/shared/metadata.yssbi");

    store.upsert(&first).await.expect("insert first record");
    assert_eq!(
        store.upsert(&second).await,
        Err(ProjectRegistryStoreError::StorageFailed)
    );
    assert_eq!(
        store.load().await.expect("load surviving record").as_ref(),
        &[first]
    );
    store.pool.close().await;
}
