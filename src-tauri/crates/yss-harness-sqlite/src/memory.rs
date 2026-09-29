use crate::SqliteHarnessStore;
use crate::codec::{decode, encode, map_insert_error, require_updated, unavailable};
use yss_harness_contract::{
    HarnessSessionId, MemoryRecord, MemoryRecordId, MemoryStatus, MemoryStorePort,
    PersistenceFailure, PersistenceFuture,
};

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

fn memory_status(status: MemoryStatus) -> &'static str {
    match status {
        MemoryStatus::Proposed => "proposed",
        MemoryStatus::Active => "active",
        MemoryStatus::Superseded => "superseded",
        MemoryStatus::Invalidated => "invalidated",
        MemoryStatus::Deleted => "deleted",
    }
}
