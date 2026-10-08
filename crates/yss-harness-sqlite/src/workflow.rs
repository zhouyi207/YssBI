use crate::SqliteHarnessStore;
use crate::codec::{conflict, decode, encode, invalid_record, map_insert_error, unavailable};
use yss_harness_contract::{
    PersistenceFailure, PersistenceFuture, WorkflowDefinition, WorkflowId, WorkflowRunId,
    WorkflowRunRecord, WorkflowRunState, WorkflowStorePort, WorkflowVersion,
};

impl WorkflowStorePort for SqliteHarnessStore {
    fn has_unfinished_runs<'a>(
        &'a self,
        session_id: &'a yss_harness_contract::HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<bool, PersistenceFailure>> {
        Box::pin(async move {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_run WHERE json_extract(payload_json, '$.sessionId') = ? AND state IN ('planned', 'ready', 'running', 'paused'))")
                .bind(session_id.as_str())
                .fetch_one(&self.pool)
                .await
                .map_err(|_| unavailable())
        })
    }

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
        expected_revision: Option<u64>,
    ) -> PersistenceFuture<'a, Result<WorkflowRunRecord, PersistenceFailure>> {
        Box::pin(async move {
            let mut committed = run.clone();
            let rows = if let Some(expected) = expected_revision {
                if run.revision != expected {
                    return Err(conflict());
                }
                committed.revision = expected.checked_add(1).ok_or_else(invalid_record)?;
                i64::try_from(committed.revision).map_err(|_| invalid_record())?;
                sqlx::query("UPDATE workflow_run SET state = ?, payload_json = ? WHERE id = ? AND json_extract(payload_json, '$.revision') = ?")
                    .bind(workflow_state(committed.state)).bind(encode(&committed)?)
                    .bind(run.id.as_str()).bind(i64::try_from(expected).map_err(|_| invalid_record())?)
                    .execute(&self.pool).await.map_err(|_| unavailable())?.rows_affected()
            } else {
                if run.revision != 0 {
                    return Err(invalid_record());
                }
                sqlx::query("INSERT INTO workflow_run (id, state, payload_json) VALUES (?, ?, ?)")
                    .bind(run.id.as_str())
                    .bind(workflow_state(run.state))
                    .bind(encode(run)?)
                    .execute(&self.pool)
                    .await
                    .map_err(map_insert_error)?
                    .rows_affected()
            };
            if rows != 1 {
                return Err(conflict());
            }
            Ok(committed)
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

fn workflow_state(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Planned => "planned",
        WorkflowRunState::Ready => "ready",
        WorkflowRunState::Running => "running",
        WorkflowRunState::Paused => "paused",
        WorkflowRunState::Completed => "completed",
        WorkflowRunState::Failed => "failed",
        WorkflowRunState::Cancelled => "cancelled",
    }
}
