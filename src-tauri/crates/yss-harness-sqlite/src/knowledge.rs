use crate::SqliteHarnessStore;
use crate::codec::{
    decode, encode, invalid_record, map_insert_error, require_updated, unavailable,
};
use yss_harness_contract::{
    KnowledgeDocumentRecord, KnowledgeSourceId, KnowledgeSourceRecord, KnowledgeSourceStatus,
    KnowledgeSourceStorePort, PersistenceFailure, PersistenceFailureCode, PersistenceFuture,
};

use sqlx::Row;

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
        updated_at: yss_harness_contract::UnixMillis,
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

fn knowledge_status(status: KnowledgeSourceStatus) -> &'static str {
    match status {
        KnowledgeSourceStatus::Active => "active",
        KnowledgeSourceStatus::Deleted => "deleted",
    }
}
