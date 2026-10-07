use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use sqlx::{Row, sqlite::SqliteRow};
use yss_harness_contract::{
    KnowledgeDocumentId, KnowledgeDocumentRecord, KnowledgeSourceId, KnowledgeSourceRecord,
    KnowledgeSourceSnapshot, KnowledgeSourceStatus, KnowledgeSourceStorePort, PersistenceFailure,
    PersistenceFailureCode, PersistenceFuture, UnixMillis,
};

use crate::SqliteHarnessStore;
use crate::codec::{decode, encode, invalid_record, map_insert_error, unavailable};

// Also invalidate on a cancelled commit: SQLite may already have committed even
// when the awaiting future is dropped. Snapshot reads acquire the sole connection
// before checking this counter, so queued commit/rollback work finishes first.
struct KnowledgeChange<'a>(&'a AtomicU64);

impl Drop for KnowledgeChange<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Release);
    }
}

impl KnowledgeSourceStorePort for SqliteHarnessStore {
    fn list_active_sources(
        &self,
    ) -> PersistenceFuture<'_, Result<Vec<KnowledgeSourceRecord>, PersistenceFailure>> {
        Box::pin(async move {
            sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM knowledge_source WHERE status = 'active' ORDER BY id",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| unavailable())?
            .into_iter()
            .map(|payload| decode(&payload))
            .collect()
        })
    }

    fn replace_source<'a>(
        &'a self,
        source: &'a KnowledgeSourceRecord,
        documents: &'a [KnowledgeDocumentRecord],
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut ids = BTreeSet::new();
            if documents
                .iter()
                .any(|document| document.source_id != source.id || !ids.insert(&document.id))
            {
                return Err(invalid_record());
            }
            let mut transaction = self.pool.begin().await.map_err(|_| unavailable())?;
            let _change = KnowledgeChange(&self.knowledge_generation);
            sqlx::query(
                "INSERT INTO knowledge_source (id, status, payload_json) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET status = excluded.status, payload_json = excluded.payload_json",
            )
            .bind(source.id.as_str())
            .bind(knowledge_status(source.status))
            .bind(encode(source)?)
            .execute(&mut *transaction)
            .await
            .map_err(|_| unavailable())?;
            sqlx::query("DELETE FROM knowledge_document WHERE source_id = ?")
                .bind(source.id.as_str())
                .execute(&mut *transaction)
                .await
                .map_err(|_| unavailable())?;
            for document in documents {
                sqlx::query(
                    "INSERT INTO knowledge_document (id, source_id, payload_json) VALUES (?, ?, ?)",
                )
                .bind(document.id.as_str())
                .bind(source.id.as_str())
                .bind(encode(document)?)
                .execute(&mut *transaction)
                .await
                .map_err(map_insert_error)?;
            }
            transaction.commit().await.map_err(|_| unavailable())
        })
    }

    fn load_active_snapshot<'a>(
        &'a self,
        known_generation: Option<u64>,
    ) -> PersistenceFuture<'a, Result<Option<KnowledgeSourceSnapshot>, PersistenceFailure>> {
        Box::pin(async move {
            let mut connection = self.pool.acquire().await.map_err(|_| unavailable())?;
            let generation = self.knowledge_generation.load(Ordering::Acquire);
            if known_generation == Some(generation) {
                return Ok(None);
            }
            let rows = sqlx::query(
                "SELECT source.payload_json AS source_json, document.payload_json AS document_json FROM knowledge_document document INNER JOIN knowledge_source source ON source.id = document.source_id WHERE source.status = 'active' ORDER BY document.id ASC",
            )
            .fetch_all(&mut *connection)
            .await
            .map_err(|_| unavailable())?;
            Ok(Some(KnowledgeSourceSnapshot {
                generation,
                documents: rows
                    .into_iter()
                    .map(decode_document)
                    .collect::<Result<_, _>>()?,
            }))
        })
    }

    fn read_active_document<'a>(
        &'a self,
        document_id: &'a KnowledgeDocumentId,
    ) -> PersistenceFuture<
        'a,
        Result<Option<(KnowledgeSourceRecord, KnowledgeDocumentRecord)>, PersistenceFailure>,
    > {
        Box::pin(async move {
            sqlx::query(
                "SELECT source.payload_json AS source_json, document.payload_json AS document_json FROM knowledge_document document INNER JOIN knowledge_source source ON source.id = document.source_id WHERE source.status = 'active' AND document.id = ?",
            )
            .bind(document_id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| unavailable())?
            .map(decode_document)
            .transpose()
        })
    }

    fn mark_source_deleted<'a>(
        &'a self,
        source_id: &'a KnowledgeSourceId,
        updated_at: UnixMillis,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut transaction = self.pool.begin().await.map_err(|_| unavailable())?;
            let _change = KnowledgeChange(&self.knowledge_generation);
            let payload = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM knowledge_source WHERE id = ?",
            )
            .bind(source_id.as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(|| PersistenceFailure::new(PersistenceFailureCode::NotFound))?;
            let mut source: KnowledgeSourceRecord = decode(&payload)?;
            source.status = KnowledgeSourceStatus::Deleted;
            source.updated_at = updated_at;
            sqlx::query(
                "UPDATE knowledge_source SET status = 'deleted', payload_json = ? WHERE id = ?",
            )
            .bind(encode(&source)?)
            .bind(source_id.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(|_| unavailable())?;
            sqlx::query("DELETE FROM knowledge_document WHERE source_id = ?")
                .bind(source_id.as_str())
                .execute(&mut *transaction)
                .await
                .map_err(|_| unavailable())?;
            transaction.commit().await.map_err(|_| unavailable())
        })
    }
}

fn decode_document(
    row: SqliteRow,
) -> Result<(KnowledgeSourceRecord, KnowledgeDocumentRecord), PersistenceFailure> {
    let source: String = row.try_get("source_json").map_err(|_| invalid_record())?;
    let document: String = row.try_get("document_json").map_err(|_| invalid_record())?;
    Ok((decode(&source)?, decode(&document)?))
}

fn knowledge_status(status: KnowledgeSourceStatus) -> &'static str {
    match status {
        KnowledgeSourceStatus::Active => "active",
        KnowledgeSourceStatus::Deleted => "deleted",
    }
}

#[cfg(test)]
mod tests;
