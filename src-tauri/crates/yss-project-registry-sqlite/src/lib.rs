//! SQLx/SQLite implementation of the project registry persistence port.

use std::path::PathBuf;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{FromRow, SqlitePool};
use std::str::FromStr;

use yss_project_identity::{ProjectRegistrationId, ProjectRootIdentity};
use yss_project_registry_contract::{
    ProjectRecord, ProjectRegistryStore, ProjectRegistryStoreError, ProjectRegistryStoreFuture,
    ProjectRootIdentityState,
};

/// The only concrete SQLx/SQLite implementation of the Project registry port.
pub struct SqliteProjectRegistryStore {
    pool: SqlitePool,
    path: PathBuf,
}

#[derive(Debug, FromRow)]
struct ProjectRegistryRow {
    id: String,
    name: String,
    path: String,
    created_at: String,
    last_opened_at: Option<String>,
    is_favorite: i64,
    root_identity: String,
    root_identity_state: String,
}

impl SqliteProjectRegistryStore {
    pub async fn connect(app_dir: PathBuf) -> Result<Self, sqlx::Error> {
        let db_path = app_dir.join("db").join("projects.sqlite");
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(sqlx::Error::Io)?;
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
    ) -> ProjectRegistryStoreFuture<'_, Result<Box<[ProjectRecord]>, ProjectRegistryStoreError>>
    {
        Box::pin(async move {
            let rows = sqlx::query_as::<_, ProjectRegistryRow>(
                "SELECT id, name, path, created_at, last_opened_at, is_favorite, root_identity, root_identity_state FROM projects",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|error| {
                tracing::warn!(
                    target: "yssbi::backend_adapters::project_registry_sqlite",
                    diagnostic_domain = "system",
                    diagnostic_event = "projectRegistryLoadFailed",
                    error = %error,
                    "Project registry load failed"
                );
                ProjectRegistryStoreError::StorageFailed
            })?;
            rows.into_iter()
                .map(row_to_record)
                .collect::<Result<Vec<_>, _>>()
                .map(Vec::into_boxed_slice)
        })
    }

    fn upsert(
        &self,
        record: &ProjectRecord,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>> {
        let id = record.id.to_string();
        let name = record.name.clone();
        let path = record.path.clone();
        let created_at = record.created_at.clone();
        let last_opened_at = record.last_opened_at.clone();
        let favorite = record.is_favorite;
        let root_identity = record.root_identity.as_str().to_owned();
        let root_identity_state = record.root_identity_state;
        Box::pin(async move {
            sqlx::query(
                "INSERT INTO projects (id, name, path, created_at, last_opened_at, is_favorite, root_identity, root_identity_state) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET name=excluded.name, path=excluded.path, created_at=excluded.created_at, last_opened_at=excluded.last_opened_at, is_favorite=excluded.is_favorite, root_identity=excluded.root_identity, root_identity_state=excluded.root_identity_state",
            )
            .bind(id)
            .bind(name)
            .bind(path)
            .bind(created_at)
            .bind(last_opened_at)
            .bind(i64::from(favorite))
            .bind(root_identity)
            .bind(match root_identity_state {
                ProjectRootIdentityState::Valid => "valid",
                ProjectRootIdentityState::Invalid => "invalid",
            })
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|error| {
                tracing::warn!(
                    target: "yssbi::backend_adapters::project_registry_sqlite",
                    diagnostic_domain = "system",
                    diagnostic_event = "projectRegistryUpsertFailed",
                    error = %error,
                    "Project registry upsert failed"
                );
                ProjectRegistryStoreError::StorageFailed
            })
        })
    }

    fn remove(
        &self,
        registration: &ProjectRegistrationId,
    ) -> ProjectRegistryStoreFuture<'_, Result<(), ProjectRegistryStoreError>> {
        let registration_id = registration.as_str().to_owned();
        Box::pin(async move {
            let result = sqlx::query("DELETE FROM projects WHERE id = ?")
                .bind(registration_id)
                .execute(&self.pool)
                .await
                .map_err(|error| {
                    tracing::warn!(
                        target: "yssbi::backend_adapters::project_registry_sqlite",
                        diagnostic_domain = "system",
                        diagnostic_event = "projectRegistryRemoveFailed",
                        error = %error,
                        "Project registry remove failed"
                    );
                    ProjectRegistryStoreError::StorageFailed
                })?;
            if result.rows_affected() == 0 {
                return Err(ProjectRegistryStoreError::Unavailable);
            }
            Ok(())
        })
    }
}

fn row_to_record(row: ProjectRegistryRow) -> Result<ProjectRecord, ProjectRegistryStoreError> {
    let state = match row.root_identity_state.as_str() {
        "valid" => ProjectRootIdentityState::Valid,
        "invalid" => ProjectRootIdentityState::Invalid,
        _ => return Err(ProjectRegistryStoreError::StorageFailed),
    };
    let favorite = match row.is_favorite {
        0 => false,
        1 => true,
        _ => return Err(ProjectRegistryStoreError::StorageFailed),
    };
    Ok(ProjectRecord {
        id: ProjectRegistrationId::from_existing(row.id),
        name: row.name,
        path: row.path,
        created_at: row.created_at,
        last_opened_at: row.last_opened_at,
        is_favorite: favorite,
        root_identity: ProjectRootIdentity::from_canonical(row.root_identity),
        root_identity_state: state,
    })
}

#[cfg(test)]
mod tests;
