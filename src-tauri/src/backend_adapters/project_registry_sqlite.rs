use std::path::PathBuf;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::str::FromStr;

use crate::project::{
    ProjectInstanceId, ProjectRegistryRecord, ProjectRegistryStore, ProjectRegistryStoreError,
    ProjectRegistryStoreFuture,
};

/// The only concrete SQLx/SQLite implementation of the Project registry port.
pub struct SqliteProjectRegistryStore {
    pool: SqlitePool,
    path: PathBuf,
}

impl SqliteProjectRegistryStore {
    pub async fn connect(app_dir: PathBuf) -> Result<Self, sqlx::Error> {
        let db_path = app_dir.join("db").join("projects.sqlite");
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| sqlx::Error::Io(error))?;
        }
        let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
        let options = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let store = Self {
            pool,
            path: db_path,
        };
        store.ensure_schema().await?;
        Ok(store)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    async fn ensure_schema(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                last_opened_at TEXT,
                is_favorite INTEGER NOT NULL DEFAULT 0,
                root_identity TEXT NOT NULL DEFAULT '',
                root_identity_state TEXT NOT NULL DEFAULT 'invalid'
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }
}

impl ProjectRegistryStore for SqliteProjectRegistryStore {
    fn load(
        &self,
    ) -> ProjectRegistryStoreFuture<
        '_,
        Result<Box<[ProjectRegistryRecord]>, ProjectRegistryStoreError>,
    > {
        Box::pin(async move {
            let _ = &self.pool;
            Err(ProjectRegistryStoreError::Unavailable)
        })
    }

    fn upsert(
        &self,
        _record: &ProjectRegistryRecord,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>> {
        Box::pin(async move { Err(ProjectRegistryStoreError::Unavailable) })
    }

    fn remove(
        &self,
        _project: &ProjectInstanceId,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>> {
        Box::pin(async move { Err(ProjectRegistryStoreError::Unavailable) })
    }
}
