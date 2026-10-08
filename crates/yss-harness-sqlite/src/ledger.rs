use crate::SqliteHarnessStore;
use crate::codec::{decode, encode, map_insert_error, require_updated, unavailable};
use yss_harness_contract::{
    HarnessSessionId, PersistenceFailure, PersistenceFuture, ToolInvocationBegin,
    ToolInvocationLedgerPort, ToolInvocationRecord, ToolInvocationState,
};

impl ToolInvocationLedgerPort for SqliteHarnessStore {
    fn load_invocation<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
        invocation_id: &'a yss_harness_contract::ToolInvocationId,
    ) -> PersistenceFuture<'a, Result<Option<ToolInvocationRecord>, PersistenceFailure>> {
        Box::pin(async move {
            let payload = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM tool_invocation WHERE session_id = ? AND id = ?",
            )
            .bind(session_id.as_str())
            .bind(invocation_id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            payload.map(|payload| decode(&payload)).transpose()
        })
    }

    fn load_running_invocations<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<ToolInvocationRecord>, PersistenceFailure>> {
        Box::pin(async {
            sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM tool_invocation WHERE state = 'running' ORDER BY rowid",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| unavailable())?
            .into_iter()
            .map(|payload| decode(&payload))
            .collect()
        })
    }

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
        let id = record.id.as_str().to_owned();
        let session_id = record.session_id.as_str().to_owned();
        let state = invocation_state(record.state);
        let payload = encode(record);
        Box::pin(async move {
            let result = sqlx::query(
                "UPDATE tool_invocation SET state = ?, payload_json = ? WHERE idempotency_key = ? AND id = ? AND session_id = ?",
            )
            .bind(state)
            .bind(payload?)
            .bind(idempotency_key)
            .bind(id)
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|_| unavailable())?;
            require_updated(result.rows_affected())
        })
    }
}

fn invocation_state(state: ToolInvocationState) -> &'static str {
    match state {
        ToolInvocationState::Running => "running",
        ToolInvocationState::Succeeded => "succeeded",
        ToolInvocationState::Failed => "failed",
    }
}
