//! Atomic plugin registry, terminal task archive, and bounded-retention operation receipts.
use std::future::Future;
use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::{Row, SqlitePool};

use crate::{
    InstalledPlugin, PluginFailure, Registry, TaskHistoryPage, TaskSnapshot, fail,
    tasks::TaskRecord,
};
use yss_plugin_protocol::{MAX_FRAME_BYTES, OPERATION_RETENTION_MS};

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(super) struct Ledger {
    runtime: Option<tokio::runtime::Runtime>,
    pool: SqlitePool,
    gate: Mutex<()>,
}

fn outside_runtime<T: Send>(
    operation: impl FnOnce() -> Result<T, PluginFailure> + Send,
) -> Result<T, PluginFailure> {
    if tokio::runtime::Handle::try_current().is_err() {
        return operation();
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("plugin-ledger".into())
            .spawn_scoped(scope, operation)
            .map_err(|_| fail("plugin_storage_failed"))?
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

impl Ledger {
    pub fn open(root: &Path) -> Result<Self, PluginFailure> {
        for name in [
            "registry.sqlite",
            "registry.sqlite-wal",
            "registry.sqlite-shm",
        ] {
            crate::storage::validate_path(root, &root.join(name))?;
        }
        outside_runtime(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| fail("plugin_storage_failed"))?;
            let options = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(root.join("registry.sqlite"))
                .create_if_missing(true)
                .foreign_keys(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Full);
            let pool = runtime
                .block_on(
                    sqlx::sqlite::SqlitePoolOptions::new()
                        .max_connections(1)
                        .connect_with(options),
                )
                .map_err(|_| fail("plugin_storage_failed"))?;
            let ledger = Self {
                runtime: Some(runtime),
                pool,
                gate: Mutex::new(()),
            };
            ledger.run(async {
                sqlx::raw_sql("CREATE TABLE IF NOT EXISTS registry (id INTEGER PRIMARY KEY CHECK(id=1), body TEXT NOT NULL);
                    CREATE TABLE IF NOT EXISTS task_archive (id TEXT PRIMARY KEY, plugin TEXT NOT NULL, operation TEXT NOT NULL,
                        digest TEXT NOT NULL, finished INTEGER NOT NULL, visible INTEGER NOT NULL DEFAULT 1, body TEXT NOT NULL, summary TEXT NOT NULL,
                        UNIQUE(plugin,operation));
                    CREATE INDEX IF NOT EXISTS task_history_page ON task_archive(plugin,visible,finished DESC,id DESC);
                    CREATE TABLE IF NOT EXISTS installation_receipts (operation TEXT PRIMARY KEY, digest TEXT NOT NULL, recorded INTEGER NOT NULL, body TEXT NOT NULL);")
                    .execute(&ledger.pool).await.map_err(|_| fail("plugin_registry_invalid"))?;
                Ok(())
            })?;
            Ok(ledger)
        })
    }

    fn run<T: Send>(
        &self,
        operation: impl Future<Output = Result<T, PluginFailure>> + Send,
    ) -> Result<T, PluginFailure> {
        outside_runtime(|| {
            let _gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
            let runtime = self
                .runtime
                .as_ref()
                .ok_or_else(|| fail("plugin_storage_failed"))?;
            let result = runtime.block_on(operation);
            // Do not keep idle SQLite handles open across Windows cleanup/move operations.
            let _ = runtime.block_on(async { self.pool.acquire().await?.close().await });
            result
        })
    }

    pub fn load(&self) -> Result<Option<Registry>, PluginFailure> {
        self.run(async {
            let body: Option<String> = sqlx::query_scalar("SELECT body FROM registry WHERE id=1")
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| fail("plugin_registry_invalid"))?;
            body.map(|body| {
                serde_json::from_str(&body).map_err(|_| fail("plugin_registry_invalid"))
            })
            .transpose()
        })
    }

    pub fn write(&self, mut next: Registry) -> Result<Registry, PluginFailure> {
        let completed = next
            .tasks
            .values()
            .filter(|task| task.snapshot.state.terminal())
            .cloned()
            .collect::<Vec<_>>();
        next.tasks.retain(|_, task| !task.snapshot.state.terminal());
        let installations = std::mem::take(&mut next.installations);
        let body = serde_json::to_string(&next).map_err(|_| fail("plugin_registry_invalid"))?;
        if body.len() > 16 * 1024 * 1024 {
            return Err(fail("plugin_resource_exhausted"));
        }
        self.run(async {
            let mut tx = self.pool.begin().await.map_err(|_| fail("plugin_storage_failed"))?;
            for task in completed {
                let mut summary = task.snapshot.clone();
                summary.result = None;
                summary.progress = None;
                if let Some(error) = &mut summary.error { error.details = None; }
                let encoded = serde_json::to_string(&task).map_err(|_| fail("plugin_registry_invalid"))?;
                if encoded.len() > MAX_FRAME_BYTES * 2 { return Err(fail("plugin_resource_exhausted")); }
                sqlx::query("INSERT INTO task_archive(id,plugin,operation,digest,finished,body,summary) VALUES(?,?,?,?,?,?,?)
                    ON CONFLICT(id) DO NOTHING")
                    .bind(&task.snapshot.task_id).bind(&task.snapshot.plugin_id).bind(&task.snapshot.operation_id)
                    .bind(&task.snapshot.package_digest).bind(now_ms() as i64).bind(encoded)
                    .bind(serde_json::to_string(&summary).map_err(|_| fail("plugin_registry_invalid"))?)
                    .execute(&mut *tx).await.map_err(|_| fail("plugin_storage_failed"))?;
            }
            for (operation, receipt) in installations {
                sqlx::query("INSERT INTO installation_receipts(operation,digest,recorded,body) VALUES(?,?,?,?) ON CONFLICT(operation) DO NOTHING")
                    .bind(operation).bind(&receipt.package_digest).bind(now_ms() as i64)
                    .bind(serde_json::to_string(&receipt).map_err(|_| fail("plugin_registry_invalid"))?)
                    .execute(&mut *tx).await.map_err(|_| fail("plugin_storage_failed"))?;
            }
            sqlx::query("INSERT INTO registry(id,body) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET body=excluded.body")
                .bind(body).execute(&mut *tx).await.map_err(|_| fail("plugin_storage_failed"))?;
            tx.commit().await.map_err(|_| fail("plugin_storage_failed"))?;
            Ok(next)
        })
    }

    pub fn task(&self, id: &str) -> Result<Option<TaskRecord>, PluginFailure> {
        self.run(async {
            let body: Option<String> =
                sqlx::query_scalar("SELECT body FROM task_archive WHERE id=?")
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|_| fail("plugin_storage_failed"))?;
            decode(body)
        })
    }
    pub fn task_operation(
        &self,
        plugin: &str,
        operation: &str,
    ) -> Result<Option<TaskRecord>, PluginFailure> {
        self.run(async {
            let body: Option<String> =
                sqlx::query_scalar("SELECT body FROM task_archive WHERE plugin=? AND operation=?")
                    .bind(plugin)
                    .bind(operation)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|_| fail("plugin_storage_failed"))?;
            decode(body)
        })
    }
    pub fn installation(&self, operation: &str) -> Result<Option<InstalledPlugin>, PluginFailure> {
        self.run(async {
            let body: Option<String> =
                sqlx::query_scalar("SELECT body FROM installation_receipts WHERE operation=?")
                    .bind(operation)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|_| fail("plugin_storage_failed"))?;
            decode(body)
        })
    }
    pub fn history(
        &self,
        plugin: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<TaskHistoryPage, PluginFailure> {
        if limit == 0 || limit > 100 {
            return Err(fail("plugin_history_query_invalid"));
        }
        let (finished, id) = match cursor {
            Some(cursor) => {
                let (time, id) = cursor
                    .split_once(':')
                    .ok_or_else(|| fail("plugin_history_query_invalid"))?;
                if !yss_plugin_protocol::valid_id(id) {
                    return Err(fail("plugin_history_query_invalid"));
                }
                (
                    time.parse::<i64>()
                        .map_err(|_| fail("plugin_history_query_invalid"))?,
                    id.to_owned(),
                )
            }
            None => (i64::MAX, String::new()),
        };
        self.run(async {
            let rows = sqlx::query("SELECT id,finished,summary FROM task_archive WHERE plugin=? AND visible=1 AND (finished<? OR (finished=? AND id<?)) ORDER BY finished DESC,id DESC LIMIT ?")
                .bind(plugin).bind(finished).bind(finished).bind(id).bind(limit as i64 + 1)
                .fetch_all(&self.pool).await.map_err(|_| fail("plugin_storage_failed"))?;
            let next_cursor = if rows.len() > limit { Some(format!("{}:{}", rows[limit-1].get::<i64,_>("finished"), rows[limit-1].get::<String,_>("id"))) } else { None };
            let tasks = rows.into_iter().take(limit).map(|row| serde_json::from_str::<TaskSnapshot>(&row.get::<String,_>("summary"))
                .map_err(|_| fail("plugin_registry_invalid"))).collect::<Result<Vec<_>, _>>()?;
            Ok(TaskHistoryPage { tasks, next_cursor })
        })
    }

    pub fn clear_history(&self, plugin: &str) -> Result<(), PluginFailure> {
        self.run(async {
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|_| fail("plugin_storage_failed"))?;
            loop {
                let records = sqlx::query(
                    "SELECT id,body FROM task_archive WHERE plugin=? AND visible=1 LIMIT 16",
                )
                .bind(plugin)
                .fetch_all(&mut *tx)
                .await
                .map_err(|_| fail("plugin_storage_failed"))?;
                if records.is_empty() {
                    break;
                }
                for row in records {
                    let mut task: TaskRecord = serde_json::from_str(&row.get::<String, _>("body"))
                        .map_err(|_| fail("plugin_registry_invalid"))?;
                    if let Some(object) = task
                        .snapshot
                        .result
                        .as_mut()
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        object.remove("viewData");
                    }
                    task.snapshot.progress = None;
                    sqlx::query("UPDATE task_archive SET visible=0,body=? WHERE id=?")
                        .bind(
                            serde_json::to_string(&task)
                                .map_err(|_| fail("plugin_registry_invalid"))?,
                        )
                        .bind(row.get::<String, _>("id"))
                        .execute(&mut *tx)
                        .await
                        .map_err(|_| fail("plugin_storage_failed"))?;
                }
            }
            tx.commit().await.map_err(|_| fail("plugin_storage_failed"))
        })
    }

    pub fn prune(&self) -> Result<(), PluginFailure> {
        let cutoff = now_ms().saturating_sub(OPERATION_RETENTION_MS) as i64;
        self.run(async {
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|_| fail("plugin_storage_failed"))?;
            sqlx::query("DELETE FROM task_archive WHERE finished<?")
                .bind(cutoff)
                .execute(&mut *tx)
                .await
                .map_err(|_| fail("plugin_storage_failed"))?;
            sqlx::query("DELETE FROM installation_receipts WHERE recorded<?")
                .bind(cutoff)
                .execute(&mut *tx)
                .await
                .map_err(|_| fail("plugin_storage_failed"))?;
            tx.commit().await.map_err(|_| fail("plugin_storage_failed"))
        })
    }

    pub fn retained_packages(&self) -> Result<Vec<String>, PluginFailure> {
        self.run(async {
            sqlx::query_scalar(
                "SELECT digest FROM installation_receipts UNION SELECT digest FROM task_archive",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| fail("plugin_storage_failed"))
        })
    }
}

fn decode<T: serde::de::DeserializeOwned>(
    body: Option<String>,
) -> Result<Option<T>, PluginFailure> {
    body.map(|body| serde_json::from_str(&body).map_err(|_| fail("plugin_registry_invalid")))
        .transpose()
}
impl Drop for Ledger {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
