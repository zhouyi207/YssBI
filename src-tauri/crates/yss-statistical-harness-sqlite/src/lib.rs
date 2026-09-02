//! SQLite persistence adapter for the statistical Harness contracts.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use yss_automation_contract::{
    ApprovalGrantId, ApprovalGrantRecord, ApprovalStorePort, HarnessEventEnvelope,
    HarnessEventStorePort, HarnessSessionId, HarnessSessionRecord, HarnessSessionState,
    HarnessSessionStorePort, HarnessTurnRecord, HarnessTurnState, KnowledgeDocumentRecord,
    KnowledgeSourceId, KnowledgeSourceRecord, KnowledgeSourceStatus, KnowledgeSourceStorePort,
    MemoryRecord, MemoryRecordId, MemoryStatus, MemoryStorePort, PersistenceFailure,
    PersistenceFailureCode, PersistenceFuture, SkillPackage, SkillSourcePort, ToolInvocationBegin,
    ToolInvocationLedgerPort, ToolInvocationRecord, ToolInvocationState, WorkflowDefinition,
    WorkflowId, WorkflowRunId, WorkflowRunRecord, WorkflowRunState, WorkflowStorePort,
    WorkflowVersion,
};

pub struct SqliteHarnessStore {
    pool: SqlitePool,
    path: Option<PathBuf>,
}

impl SqliteHarnessStore {
    pub async fn connect(app_dir: PathBuf) -> Result<Self, PersistenceFailure> {
        let path = app_dir.join("db").join("statistical-harness.sqlite");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| unavailable())?;
        }
        let url = sqlite_url(&path);
        let options = SqliteConnectOptions::from_str(&url)
            .map_err(|_| unavailable())?
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
        let store = Self { pool, path };
        store.ensure_schema().await?;
        Ok(store)
    }

    async fn ensure_schema(&self) -> Result<(), PersistenceFailure> {
        for statement in SCHEMA {
            sqlx::query(*statement)
                .execute(&self.pool)
                .await
                .map_err(|_| unavailable())?;
        }
        Ok(())
    }
}

const SCHEMA: &[&str] = &[
    r#"CREATE TABLE IF NOT EXISTS assistant_session (
        id TEXT PRIMARY KEY NOT NULL,
        state TEXT NOT NULL,
        payload_json TEXT NOT NULL
    )"#,
    r#"CREATE TABLE IF NOT EXISTS assistant_turn (
        id TEXT PRIMARY KEY NOT NULL,
        session_id TEXT NOT NULL,
        state TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        FOREIGN KEY(session_id) REFERENCES assistant_session(id)
    )"#,
    r#"CREATE TABLE IF NOT EXISTS assistant_event (
        session_id TEXT NOT NULL,
        sequence INTEGER NOT NULL,
        payload_json TEXT NOT NULL,
        PRIMARY KEY(session_id, sequence),
        FOREIGN KEY(session_id) REFERENCES assistant_session(id)
    )"#,
    r#"CREATE TABLE IF NOT EXISTS workflow_definition (
        id TEXT NOT NULL,
        version TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        PRIMARY KEY(id, version)
    )"#,
    r#"CREATE TABLE IF NOT EXISTS workflow_run (
        id TEXT PRIMARY KEY NOT NULL,
        state TEXT NOT NULL,
        payload_json TEXT NOT NULL
    )"#,
    r#"CREATE TABLE IF NOT EXISTS tool_invocation (
        id TEXT PRIMARY KEY NOT NULL,
        idempotency_key TEXT NOT NULL UNIQUE,
        session_id TEXT NOT NULL,
        state TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        FOREIGN KEY(session_id) REFERENCES assistant_session(id)
    )"#,
    r#"CREATE TABLE IF NOT EXISTS memory_record (
        id TEXT PRIMARY KEY NOT NULL,
        session_id TEXT NOT NULL,
        status TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        FOREIGN KEY(session_id) REFERENCES assistant_session(id)
    )"#,
    r#"CREATE TABLE IF NOT EXISTS knowledge_source (
        id TEXT PRIMARY KEY NOT NULL,
        status TEXT NOT NULL,
        payload_json TEXT NOT NULL
    )"#,
    r#"CREATE TABLE IF NOT EXISTS knowledge_document (
        id TEXT PRIMARY KEY NOT NULL,
        source_id TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        FOREIGN KEY(source_id) REFERENCES knowledge_source(id)
    )"#,
    r#"CREATE TABLE IF NOT EXISTS skill_installation (
        id TEXT NOT NULL,
        version TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        PRIMARY KEY(id, version)
    )"#,
    r#"CREATE TABLE IF NOT EXISTS approval_grant (
        id TEXT PRIMARY KEY NOT NULL,
        consumed_at INTEGER,
        payload_json TEXT NOT NULL
    )"#,
];

impl HarnessSessionStorePort for SqliteHarnessStore {
    fn create_session<'a>(
        &'a self,
        record: &'a HarnessSessionRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = record.id.as_str().to_owned();
        let state = session_state(record.state);
        let payload = encode(record);
        Box::pin(async move {
            sqlx::query("INSERT INTO assistant_session (id, state, payload_json) VALUES (?, ?, ?)")
                .bind(id)
                .bind(state)
                .bind(payload?)
                .execute(&self.pool)
                .await
                .map(|_| ())
                .map_err(map_insert_error)
        })
    }

    fn load_session<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<Option<HarnessSessionRecord>, PersistenceFailure>> {
        let id = session_id.as_str().to_owned();
        Box::pin(async move {
            let payload = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM assistant_session WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            payload.map(|payload| decode(&payload)).transpose()
        })
    }

    fn load_open_sessions<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessSessionRecord>, PersistenceFailure>> {
        Box::pin(async move {
            let payloads = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM assistant_session WHERE state IN ('active', 'closing') ORDER BY id ASC",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            payloads
                .into_iter()
                .map(|payload| decode(&payload))
                .collect()
        })
    }

    fn update_session<'a>(
        &'a self,
        record: &'a HarnessSessionRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = record.id.as_str().to_owned();
        let state = session_state(record.state);
        let payload = encode(record);
        Box::pin(async move {
            let result = sqlx::query(
                "UPDATE assistant_session SET state = ?, payload_json = ? WHERE id = ?",
            )
            .bind(state)
            .bind(payload?)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            require_updated(result.rows_affected())
        })
    }

    fn create_turn<'a>(
        &'a self,
        record: &'a HarnessTurnRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = record.id.as_str().to_owned();
        let session_id = record.session_id.as_str().to_owned();
        let state = turn_state(record.state);
        let payload = encode(record);
        Box::pin(async move {
            sqlx::query(
                "INSERT INTO assistant_turn (id, session_id, state, payload_json) VALUES (?, ?, ?, ?)",
            )
            .bind(id)
            .bind(session_id)
            .bind(state)
            .bind(payload?)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(map_insert_error)
        })
    }

    fn load_turn<'a>(
        &'a self,
        turn_id: &'a yss_automation_contract::HarnessTurnId,
    ) -> PersistenceFuture<'a, Result<Option<HarnessTurnRecord>, PersistenceFailure>> {
        let id = turn_id.as_str().to_owned();
        Box::pin(async move {
            sqlx::query_scalar::<_, String>("SELECT payload_json FROM assistant_turn WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| unavailable())?
                .map(|payload| decode(&payload))
                .transpose()
        })
    }

    fn update_turn<'a>(
        &'a self,
        record: &'a HarnessTurnRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = record.id.as_str().to_owned();
        let state = turn_state(record.state);
        let payload = encode(record);
        Box::pin(async move {
            let result =
                sqlx::query("UPDATE assistant_turn SET state = ?, payload_json = ? WHERE id = ?")
                    .bind(state)
                    .bind(payload?)
                    .bind(id)
                    .execute(&self.pool)
                    .await
                    .map_err(|_| unavailable())?;
            require_updated(result.rows_affected())
        })
    }
}

impl HarnessEventStorePort for SqliteHarnessStore {
    fn append_event<'a>(
        &'a self,
        event: &'a HarnessEventEnvelope,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let session_id = event.session_id.as_str().to_owned();
        let sequence = i64::try_from(event.sequence).map_err(|_| invalid_record());
        let payload = encode(event);
        Box::pin(async move {
            sqlx::query(
                "INSERT INTO assistant_event (session_id, sequence, payload_json) VALUES (?, ?, ?)",
            )
            .bind(session_id)
            .bind(sequence?)
            .bind(payload?)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(map_insert_error)
        })
    }

    fn load_events_after<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
        sequence: u64,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessEventEnvelope>, PersistenceFailure>> {
        let id = session_id.as_str().to_owned();
        let sequence = i64::try_from(sequence).map_err(|_| invalid_record());
        Box::pin(async move {
            let payloads = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM assistant_event WHERE session_id = ? AND sequence > ? ORDER BY sequence ASC",
            )
            .bind(id)
            .bind(sequence?)
            .fetch_all(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            payloads
                .into_iter()
                .map(|payload| decode(&payload))
                .collect()
        })
    }

    fn latest_sequence<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<u64, PersistenceFailure>> {
        let id = session_id.as_str().to_owned();
        Box::pin(async move {
            let row = sqlx::query(
                "SELECT COALESCE(MAX(sequence), 0) AS sequence FROM assistant_event WHERE session_id = ?",
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            let sequence: i64 = row.try_get("sequence").map_err(|_| invalid_record())?;
            u64::try_from(sequence).map_err(|_| invalid_record())
        })
    }
}

impl WorkflowStorePort for SqliteHarnessStore {
    fn save_definition<'a>(
        &'a self,
        definition: &'a WorkflowDefinition,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = definition.id.as_str().to_owned();
        let version = definition.version.as_str().to_owned();
        let payload = encode(definition);
        Box::pin(async move {
            let payload = payload?;
            let result = sqlx::query(
                "INSERT INTO workflow_definition (id, version, payload_json) VALUES (?, ?, ?) ON CONFLICT(id, version) DO NOTHING",
            )
            .bind(&id)
            .bind(&version)
            .bind(&payload)
            .execute(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            if result.rows_affected() == 1 {
                return Ok(());
            }
            let existing = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM workflow_definition WHERE id = ? AND version = ?",
            )
            .bind(id)
            .bind(version)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            if existing == payload {
                Ok(())
            } else {
                Err(conflict())
            }
        })
    }

    fn load_definition<'a>(
        &'a self,
        id: &'a WorkflowId,
        version: &'a WorkflowVersion,
    ) -> PersistenceFuture<'a, Result<Option<WorkflowDefinition>, PersistenceFailure>> {
        let id = id.as_str().to_owned();
        let version = version.as_str().to_owned();
        Box::pin(async move {
            sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM workflow_definition WHERE id = ? AND version = ?",
            )
            .bind(id)
            .bind(version)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| unavailable())?
            .map(|payload| decode(&payload))
            .transpose()
        })
    }

    fn save_run<'a>(
        &'a self,
        run: &'a WorkflowRunRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = run.id.as_str().to_owned();
        let state = workflow_state(run.state);
        let payload = encode(run);
        Box::pin(async move {
            sqlx::query(
                "INSERT INTO workflow_run (id, state, payload_json) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET state = excluded.state, payload_json = excluded.payload_json",
            )
            .bind(id)
            .bind(state)
            .bind(payload?)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|_| unavailable())
        })
    }

    fn load_run<'a>(
        &'a self,
        id: &'a WorkflowRunId,
    ) -> PersistenceFuture<'a, Result<Option<WorkflowRunRecord>, PersistenceFailure>> {
        let id = id.as_str().to_owned();
        Box::pin(async move {
            sqlx::query_scalar::<_, String>("SELECT payload_json FROM workflow_run WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| unavailable())?
                .map(|payload| decode(&payload))
                .transpose()
        })
    }

    fn load_recoverable_runs<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<WorkflowRunRecord>, PersistenceFailure>> {
        Box::pin(async move {
            let payloads = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM workflow_run WHERE state IN ('running', 'paused', 'ready') ORDER BY id ASC",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            payloads
                .into_iter()
                .map(|payload| decode(&payload))
                .collect()
        })
    }
}

impl ToolInvocationLedgerPort for SqliteHarnessStore {
    fn begin<'a>(
        &'a self,
        record: &'a ToolInvocationRecord,
    ) -> PersistenceFuture<'a, Result<ToolInvocationBegin, PersistenceFailure>> {
        let id = record.id.as_str().to_owned();
        let idempotency_key = record.idempotency_key.as_str().to_owned();
        let session_id = record.session_id.as_str().to_owned();
        let state = invocation_state(record.state);
        let payload = encode(record);
        Box::pin(async move {
            let result = sqlx::query(
                "INSERT INTO tool_invocation (id, idempotency_key, session_id, state, payload_json) VALUES (?, ?, ?, ?, ?) ON CONFLICT(idempotency_key) DO NOTHING",
            )
            .bind(id)
            .bind(&idempotency_key)
            .bind(session_id)
            .bind(state)
            .bind(payload?)
            .execute(&self.pool)
            .await
            .map_err(map_insert_error)?;
            if result.rows_affected() == 1 {
                return Ok(ToolInvocationBegin::Started);
            }
            let existing = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM tool_invocation WHERE idempotency_key = ?",
            )
            .bind(idempotency_key)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            decode(&existing).map(|record| ToolInvocationBegin::Existing(Box::new(record)))
        })
    }

    fn finish<'a>(
        &'a self,
        record: &'a ToolInvocationRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let idempotency_key = record.idempotency_key.as_str().to_owned();
        let state = invocation_state(record.state);
        let payload = encode(record);
        Box::pin(async move {
            let result = sqlx::query(
                "UPDATE tool_invocation SET state = ?, payload_json = ? WHERE idempotency_key = ?",
            )
            .bind(state)
            .bind(payload?)
            .bind(idempotency_key)
            .execute(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            require_updated(result.rows_affected())
        })
    }
}

impl ApprovalStorePort for SqliteHarnessStore {
    fn insert<'a>(
        &'a self,
        record: &'a ApprovalGrantRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = record.id.as_str().to_owned();
        let payload = encode(record);
        Box::pin(async move {
            sqlx::query(
                "INSERT INTO approval_grant (id, consumed_at, payload_json) VALUES (?, NULL, ?)",
            )
            .bind(id)
            .bind(payload?)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(map_insert_error)
        })
    }

    fn load<'a>(
        &'a self,
        id: &'a ApprovalGrantId,
    ) -> PersistenceFuture<'a, Result<Option<ApprovalGrantRecord>, PersistenceFailure>> {
        let id = id.as_str().to_owned();
        Box::pin(async move {
            sqlx::query_scalar::<_, String>("SELECT payload_json FROM approval_grant WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| unavailable())?
                .map(|payload| decode(&payload))
                .transpose()
        })
    }

    fn consume<'a>(
        &'a self,
        id: &'a ApprovalGrantId,
        consumed_at: yss_automation_contract::UnixMillis,
    ) -> PersistenceFuture<'a, Result<bool, PersistenceFailure>> {
        let id = id.as_str().to_owned();
        Box::pin(async move {
            let payload = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM approval_grant WHERE id = ?",
            )
            .bind(&id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(|| PersistenceFailure::new(PersistenceFailureCode::NotFound))?;
            let mut record: ApprovalGrantRecord = decode(&payload)?;
            if record.consumed_at.is_some() {
                return Ok(false);
            }
            record.consumed_at = Some(consumed_at);
            let consumed_at = i64::try_from(consumed_at.get()).map_err(|_| invalid_record())?;
            let result = sqlx::query(
                "UPDATE approval_grant SET consumed_at = ?, payload_json = ? WHERE id = ? AND consumed_at IS NULL",
            )
            .bind(consumed_at)
            .bind(encode(&record)?)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            Ok(result.rows_affected() == 1)
        })
    }
}

impl MemoryStorePort for SqliteHarnessStore {
    fn insert<'a>(
        &'a self,
        record: &'a MemoryRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = record.id.as_str().to_owned();
        let session_id = record.session_id.as_str().to_owned();
        let status = memory_status(record.status);
        let payload = encode(record);
        Box::pin(async move {
            sqlx::query(
                "INSERT INTO memory_record (id, session_id, status, payload_json) VALUES (?, ?, ?, ?)",
            )
            .bind(id)
            .bind(session_id)
            .bind(status)
            .bind(payload?)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(map_insert_error)
        })
    }

    fn load<'a>(
        &'a self,
        id: &'a MemoryRecordId,
    ) -> PersistenceFuture<'a, Result<Option<MemoryRecord>, PersistenceFailure>> {
        let id = id.as_str().to_owned();
        Box::pin(async move {
            sqlx::query_scalar::<_, String>("SELECT payload_json FROM memory_record WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| unavailable())?
                .map(|payload| decode(&payload))
                .transpose()
        })
    }

    fn update<'a>(
        &'a self,
        record: &'a MemoryRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = record.id.as_str().to_owned();
        let status = memory_status(record.status);
        let payload = encode(record);
        Box::pin(async move {
            let result =
                sqlx::query("UPDATE memory_record SET status = ?, payload_json = ? WHERE id = ?")
                    .bind(status)
                    .bind(payload?)
                    .bind(id)
                    .execute(&self.pool)
                    .await
                    .map_err(|_| unavailable())?;
            require_updated(result.rows_affected())
        })
    }

    fn activate<'a>(
        &'a self,
        record: &'a MemoryRecord,
        superseded: Option<&'a MemoryRecord>,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let record_id = record.id.as_str().to_owned();
        let record_status = memory_status(record.status);
        let record_payload = encode(record);
        let superseded = superseded.map(|record| {
            (
                record.id.as_str().to_owned(),
                memory_status(record.status),
                encode(record),
            )
        });
        Box::pin(async move {
            let mut transaction = self.pool.begin().await.map_err(|_| unavailable())?;
            if let Some((id, status, payload)) = superseded {
                let result = sqlx::query(
                    "UPDATE memory_record SET status = ?, payload_json = ? WHERE id = ? AND status = 'active'",
                )
                .bind(status)
                .bind(payload?)
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(|_| unavailable())?;
                require_updated(result.rows_affected())?;
            }
            let result = sqlx::query(
                "UPDATE memory_record SET status = ?, payload_json = ? WHERE id = ? AND status = 'proposed'",
            )
            .bind(record_status)
            .bind(record_payload?)
            .bind(record_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| unavailable())?;
            require_updated(result.rows_affected())?;
            transaction.commit().await.map_err(|_| unavailable())
        })
    }

    fn query_session<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<Vec<MemoryRecord>, PersistenceFailure>> {
        let session_id = session_id.as_str().to_owned();
        Box::pin(async move {
            let payloads = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM memory_record WHERE session_id = ? ORDER BY id ASC",
            )
            .bind(session_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            payloads
                .into_iter()
                .map(|payload| decode(&payload))
                .collect()
        })
    }

    fn list_active<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<MemoryRecord>, PersistenceFailure>> {
        Box::pin(async move {
            let payloads = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM memory_record WHERE status = 'active' ORDER BY id ASC",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            payloads
                .into_iter()
                .map(|payload| decode(&payload))
                .collect()
        })
    }
}

impl KnowledgeSourceStorePort for SqliteHarnessStore {
    fn upsert_source<'a>(
        &'a self,
        source: &'a KnowledgeSourceRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = source.id.as_str().to_owned();
        let status = knowledge_status(source.status);
        let payload = encode(source);
        Box::pin(async move {
            sqlx::query(
                "INSERT INTO knowledge_source (id, status, payload_json) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET status = excluded.status, payload_json = excluded.payload_json",
            )
            .bind(id)
            .bind(status)
            .bind(payload?)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|_| unavailable())
        })
    }

    fn upsert_document<'a>(
        &'a self,
        document: &'a KnowledgeDocumentRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = document.id.as_str().to_owned();
        let source_id = document.source_id.as_str().to_owned();
        let payload = encode(document);
        Box::pin(async move {
            sqlx::query(
                "INSERT INTO knowledge_document (id, source_id, payload_json) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_id = excluded.source_id, payload_json = excluded.payload_json",
            )
            .bind(id)
            .bind(source_id)
            .bind(payload?)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(map_insert_error)
        })
    }

    fn list_active_documents<'a>(
        &'a self,
    ) -> PersistenceFuture<
        'a,
        Result<Vec<(KnowledgeSourceRecord, KnowledgeDocumentRecord)>, PersistenceFailure>,
    > {
        Box::pin(async move {
            let rows = sqlx::query(
                "SELECT source.payload_json AS source_json, document.payload_json AS document_json FROM knowledge_document document INNER JOIN knowledge_source source ON source.id = document.source_id WHERE source.status = 'active' ORDER BY document.id ASC",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            rows.into_iter()
                .map(|row| {
                    let source: String =
                        row.try_get("source_json").map_err(|_| invalid_record())?;
                    let document: String =
                        row.try_get("document_json").map_err(|_| invalid_record())?;
                    Ok((decode(&source)?, decode(&document)?))
                })
                .collect()
        })
    }

    fn mark_source_deleted<'a>(
        &'a self,
        source_id: &'a KnowledgeSourceId,
        updated_at: yss_automation_contract::UnixMillis,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let source_id = source_id.as_str().to_owned();
        Box::pin(async move {
            let payload = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM knowledge_source WHERE id = ?",
            )
            .bind(&source_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(|| PersistenceFailure::new(PersistenceFailureCode::NotFound))?;
            let mut source: KnowledgeSourceRecord = decode(&payload)?;
            source.status = KnowledgeSourceStatus::Deleted;
            source.updated_at = updated_at;
            let result = sqlx::query(
                "UPDATE knowledge_source SET status = 'deleted', payload_json = ? WHERE id = ?",
            )
            .bind(encode(&source)?)
            .bind(source_id)
            .execute(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            require_updated(result.rows_affected())
        })
    }
}

impl SkillSourcePort for SqliteHarnessStore {
    fn install_package<'a>(
        &'a self,
        package: &'a SkillPackage,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        let id = package.manifest.id.as_str().to_owned();
        let version = package.manifest.version.as_str().to_owned();
        let payload = encode(package);
        Box::pin(async move {
            let payload = payload?;
            let result = sqlx::query(
                "INSERT INTO skill_installation (id, version, payload_json) VALUES (?, ?, ?) ON CONFLICT(id, version) DO NOTHING",
            )
            .bind(&id)
            .bind(&version)
            .bind(&payload)
            .execute(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            if result.rows_affected() == 1 {
                return Ok(());
            }
            let existing = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM skill_installation WHERE id = ? AND version = ?",
            )
            .bind(id)
            .bind(version)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            if existing == payload {
                Ok(())
            } else {
                Err(conflict())
            }
        })
    }

    fn list_packages<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<SkillPackage>, PersistenceFailure>> {
        Box::pin(async move {
            let payloads = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM skill_installation ORDER BY id ASC, version ASC",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            payloads
                .into_iter()
                .map(|payload| decode(&payload))
                .collect()
        })
    }
}

fn encode<T: serde::Serialize>(value: &T) -> Result<String, PersistenceFailure> {
    serde_json::to_string(value).map_err(|_| invalid_record())
}

fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, PersistenceFailure> {
    serde_json::from_str(value).map_err(|_| invalid_record())
}

fn sqlite_url(path: &Path) -> String {
    format!("sqlite://{}", path.to_string_lossy().replace('\\', "/"))
}

fn session_state(state: HarnessSessionState) -> &'static str {
    match state {
        HarnessSessionState::Active => "active",
        HarnessSessionState::Closing => "closing",
        HarnessSessionState::Stale => "stale",
        HarnessSessionState::Closed => "closed",
    }
}

fn turn_state(state: HarnessTurnState) -> &'static str {
    match state {
        HarnessTurnState::Running => "running",
        HarnessTurnState::Completed => "completed",
        HarnessTurnState::Failed => "failed",
        HarnessTurnState::Cancelled => "cancelled",
    }
}

fn workflow_state(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Planned => "planned",
        WorkflowRunState::WaitingForApproval => "waiting_for_approval",
        WorkflowRunState::Ready => "ready",
        WorkflowRunState::Running => "running",
        WorkflowRunState::Paused => "paused",
        WorkflowRunState::WaitingForExternalInput => "waiting_for_external_input",
        WorkflowRunState::Completed => "completed",
        WorkflowRunState::Failed => "failed",
        WorkflowRunState::Cancelled => "cancelled",
    }
}

fn invocation_state(state: ToolInvocationState) -> &'static str {
    match state {
        ToolInvocationState::Running => "running",
        ToolInvocationState::Succeeded => "succeeded",
        ToolInvocationState::Failed => "failed",
    }
}

fn memory_status(status: MemoryStatus) -> &'static str {
    match status {
        MemoryStatus::Proposed => "proposed",
        MemoryStatus::Approved => "approved",
        MemoryStatus::Active => "active",
        MemoryStatus::Superseded => "superseded",
        MemoryStatus::Invalidated => "invalidated",
        MemoryStatus::Deleted => "deleted",
    }
}

fn knowledge_status(status: KnowledgeSourceStatus) -> &'static str {
    match status {
        KnowledgeSourceStatus::Active => "active",
        KnowledgeSourceStatus::Deleted => "deleted",
    }
}

fn require_updated(rows: u64) -> Result<(), PersistenceFailure> {
    if rows == 1 {
        Ok(())
    } else {
        Err(PersistenceFailure::new(PersistenceFailureCode::NotFound))
    }
}

fn map_insert_error(error: sqlx::Error) -> PersistenceFailure {
    if error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
    {
        conflict()
    } else {
        unavailable()
    }
}

fn conflict() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::Conflict)
}

fn unavailable() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::Unavailable)
}

fn invalid_record() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::InvalidRecord)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_automation_contract::{
        ApprovalGrantId, ApprovalGrantRecord, ApprovalStorePort, AutomationCapabilityRequest,
        HarnessEvent, HarnessSessionState, IdempotencyKey, InspectGraphRequest, PrincipalId,
        ProjectSessionBinding, SourceHash, ToolInvocationId, ToolInvocationState, UnixMillis,
    };
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    #[tokio::test]
    async fn sqlite_enforces_event_sequence_and_tool_idempotency() {
        let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
        let project = ProjectSessionBinding::new(
            ProjectInstanceId::from_existing("project-1".into()),
            ProjectSessionId::new("project-session-1"),
        );
        let session = HarnessSessionRecord {
            id: HarnessSessionId::try_new("session-1").unwrap(),
            principal_id: PrincipalId::try_new("user-1").unwrap(),
            project: project.clone(),
            state: HarnessSessionState::Active,
            created_at: UnixMillis::from_existing(10),
            updated_at: UnixMillis::from_existing(10),
        };
        store.create_session(&session).await.unwrap();
        let event = HarnessEventEnvelope {
            sequence: 1,
            session_id: session.id.clone(),
            turn_id: None,
            occurred_at: UnixMillis::from_existing(11),
            event: HarnessEvent::SessionCreated,
        };
        store.append_event(&event).await.unwrap();
        assert_eq!(
            store.append_event(&event).await.unwrap_err().code,
            PersistenceFailureCode::Conflict
        );

        let invocation = ToolInvocationRecord {
            id: ToolInvocationId::try_new("tool-1").unwrap(),
            idempotency_key: IdempotencyKey::try_new("idem-1").unwrap(),
            session_id: session.id.clone(),
            turn_id: yss_automation_contract::HarnessTurnId::try_new("turn-1").unwrap(),
            workflow_run_id: None,
            workflow_step_id: None,
            project,
            capability_id: yss_automation_contract::CapabilityId::InspectGraph,
            request: AutomationCapabilityRequest::InspectGraph(InspectGraphRequest {
                graph_path: "events/Main.yssbi-event".to_owned(),
            }),
            state: ToolInvocationState::Running,
            result: None,
            failure: None,
            started_at: UnixMillis::from_existing(12),
            deadline: UnixMillis::from_existing(42),
            finished_at: None,
        };
        assert!(matches!(
            store.begin(&invocation).await.unwrap(),
            ToolInvocationBegin::Started
        ));
        assert!(matches!(
            store.begin(&invocation).await.unwrap(),
            ToolInvocationBegin::Existing(existing) if *existing == invocation
        ));
        assert_eq!(store.latest_sequence(&session.id).await.unwrap(), 1);
        assert_eq!(
            store.load_session(&session.id).await.unwrap(),
            Some(session.clone())
        );

        let grant = ApprovalGrantRecord {
            id: ApprovalGrantId::try_new("approval-1").unwrap(),
            principal_id: session.principal_id.clone(),
            session_id: session.id,
            project: invocation.project.clone(),
            capability_id: yss_automation_contract::CapabilityId::ApplyGraphEdit,
            request_fingerprint: SourceHash::try_new("fingerprint-1").unwrap(),
            issued_at: UnixMillis::from_existing(13),
            expires_at: UnixMillis::from_existing(100),
            consumed_at: None,
        };
        ApprovalStorePort::insert(&store, &grant).await.unwrap();
        assert!(
            ApprovalStorePort::consume(&store, &grant.id, UnixMillis::from_existing(14))
                .await
                .unwrap()
        );
        assert!(
            !ApprovalStorePort::consume(&store, &grant.id, UnixMillis::from_existing(15))
                .await
                .unwrap()
        );
        assert_eq!(
            ApprovalStorePort::load(&store, &grant.id)
                .await
                .unwrap()
                .unwrap()
                .consumed_at,
            Some(UnixMillis::from_existing(14))
        );
    }
}
