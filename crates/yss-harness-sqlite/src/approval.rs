use crate::SqliteHarnessStore;
use crate::codec::{decode, encode, invalid_record, map_insert_error, unavailable};
use yss_harness_contract::{
    ApprovalGrantId, ApprovalGrantRecord, ApprovalStorePort, PersistenceFailure,
    PersistenceFailureCode, PersistenceFuture,
};

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
        consumed_at: yss_harness_contract::UnixMillis,
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
