use crate::SqliteHarnessStore;
use crate::codec::{decode, encode, invalid_record, map_insert_error, unavailable};
use yss_harness_contract::{
    HarnessEvent, HarnessEventEnvelope, HarnessEventStorePort, HarnessSessionId, HarnessTurnId,
    PersistenceFailure, PersistenceFuture, UnixMillis,
};

use sqlx::Row;

impl HarnessEventStorePort for SqliteHarnessStore {
    fn append_event<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
        turn_id: Option<&'a HarnessTurnId>,
        occurred_at: UnixMillis,
        event: HarnessEvent,
    ) -> PersistenceFuture<'a, Result<HarnessEventEnvelope, PersistenceFailure>> {
        Box::pin(async move {
            // Take the database write reservation before reading the head. This
            // also serializes allocators using different pools or Host instances.
            let mut transaction = self
                .pool
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(|_| unavailable())?;
            let previous = sqlx::query_scalar::<_, i64>(
                "SELECT COALESCE(MAX(sequence), 0) FROM assistant_event WHERE session_id = ?",
            )
            .bind(session_id.as_str())
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| unavailable())?;
            let sequence = previous
                .checked_add(1)
                .filter(|value| *value > 0)
                .ok_or_else(invalid_record)?;
            let envelope = HarnessEventEnvelope {
                sequence: sequence as u64,
                session_id: session_id.clone(),
                turn_id: turn_id.cloned(),
                occurred_at,
                event,
            };
            sqlx::query(
                "INSERT INTO assistant_event (session_id, sequence, payload_json) VALUES (?, ?, ?)",
            )
            .bind(session_id.as_str())
            .bind(sequence)
            .bind(encode(&envelope)?)
            .execute(&mut *transaction)
            .await
            .map_err(map_insert_error)?;
            transaction.commit().await.map_err(|_| unavailable())?;
            Ok(envelope)
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
