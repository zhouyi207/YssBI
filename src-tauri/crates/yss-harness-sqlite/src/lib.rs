//! SQLite persistence adapter for the statistical Harness contracts.

#![forbid(unsafe_code)]

mod approval;
mod codec;
mod events;
mod knowledge;
mod ledger;
mod schema;
mod session;
mod workflow;

use codec::{invalid_record, unavailable};
use schema::SCHEMA;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::path::{Path, PathBuf};
#[cfg(any(test, feature = "test-support"))]
use std::str::FromStr;
use yss_harness_contract::PersistenceFailure;

pub struct SqliteHarnessStore {
    pool: SqlitePool,
    path: Option<PathBuf>,
    knowledge_generation: std::sync::atomic::AtomicU64,
}

impl SqliteHarnessStore {
    pub async fn connect(app_dir: PathBuf) -> Result<Self, PersistenceFailure> {
        let path = app_dir.join("db").join("statistical-harness.sqlite");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| unavailable())?;
        }
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
        Self::connect_with_options(options, Some(path)).await
    }

    #[cfg(any(test, feature = "test-support"))]
    pub async fn connect_in_memory() -> Result<Self, PersistenceFailure> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(|_| unavailable())?
            .foreign_keys(true);
        Self::connect_with_options(options, None).await
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    async fn connect_with_options(
        options: SqliteConnectOptions,
        path: Option<PathBuf>,
    ) -> Result<Self, PersistenceFailure> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|_| unavailable())?;
        let store = Self {
            pool,
            path,
            knowledge_generation: Default::default(),
        };
        store.ensure_schema().await?;
        Ok(store)
    }

    async fn ensure_schema(&self) -> Result<(), PersistenceFailure> {
        let mut transaction = self.pool.begin().await.map_err(|_| unavailable())?;
        let mut existing: Vec<String> = sqlx::query_scalar(
            "SELECT sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' AND sql IS NOT NULL",
        )
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| unavailable())?;
        if existing.is_empty() {
            for statement in SCHEMA {
                sqlx::query(*statement)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|_| unavailable())?;
            }
        } else {
            // This adapter owns the entire schema. Reject incompatible databases
            // without rewriting payloads or implicitly rebuilding durable history.
            existing.sort();
            let mut expected: Vec<String> = SCHEMA.iter().map(|sql| (*sql).to_owned()).collect();
            expected.sort();
            if existing != expected {
                return Err(invalid_record());
            }
        }
        transaction.commit().await.map_err(|_| unavailable())
    }
}

#[cfg(test)]
mod concurrency_tests;
#[cfg(test)]
mod tests;
