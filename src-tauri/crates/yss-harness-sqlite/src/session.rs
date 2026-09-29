use crate::SqliteHarnessStore;
use crate::codec::{decode, encode, map_insert_error, require_updated, unavailable};
use yss_harness_contract::{
    HarnessSessionId, HarnessSessionRecord, HarnessSessionState, HarnessSessionStorePort,
    HarnessTurnRecord, HarnessTurnState, PersistenceFailure, PersistenceFuture,
};

impl HarnessSessionStorePort for SqliteHarnessStore {
    fn list_conversations<'a>(
        &'a self,
        principal: &'a yss_harness_contract::PrincipalId,
        project_key: &'a str,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessSessionRecord>, PersistenceFailure>> {
        Box::pin(async move {
            sqlx::query_scalar::<_, String>("SELECT payload_json FROM assistant_session WHERE json_extract(payload_json, '$.principalId') = ? AND json_extract(payload_json, '$.conversation.projectKey') = ? ORDER BY rowid")
                .bind(principal.as_str()).bind(project_key).fetch_all(&self.pool).await.map_err(|_| unavailable())?
                .into_iter().map(|payload| decode(&payload)).collect()
        })
    }

    fn load_running_turns<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessTurnRecord>, PersistenceFailure>> {
        Box::pin(async {
            sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM assistant_turn WHERE state = 'running' ORDER BY rowid",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| unavailable())?
            .into_iter()
            .map(|payload| decode(&payload))
            .collect()
        })
    }

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
        turn_id: &'a yss_harness_contract::HarnessTurnId,
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
