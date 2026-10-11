//! Logging-owned retention, independent of host preferences and collection filters.

use sqlx::{Connection, Row, SqliteConnection};

use crate::LogStoreError;

const MIB: u64 = 1024 * 1024;
const DELETE_BUDGET: usize = 512;
const SIZE_DELETE_BATCH: usize = 32;

/// Optional history eviction. Defaults retain all records.
///
/// Age compares the stored local wall-clock timestamp with the current local time,
/// preserving records exactly on the cutoff. Storage targets allocated SQLite
/// pages (including indexes), not a hard filesystem cap. Oldest sequences are
/// removed first. Free pages and WAL space are reclaimed incrementally; readers,
/// fragmentation, database overhead and pending work can keep files above target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LogRetentionPolicy {
    days: Option<u32>,
    storage_mib: Option<u32>,
}

impl LogRetentionPolicy {
    pub fn new(days: Option<u32>, storage_mib: Option<u32>) -> Result<Self, LogStoreError> {
        if days.is_some_and(|value| !(1..=3650).contains(&value))
            || storage_mib.is_some_and(|value| !(1..=10240).contains(&value))
        {
            return Err(LogStoreError::InvalidRetention);
        }
        Ok(Self { days, storage_mib })
    }

    pub(crate) fn is_enabled(self) -> bool {
        self.days.is_some() || self.storage_mib.is_some()
    }
}

pub(crate) struct RetentionOutcome {
    pub evicted: Vec<u64>,
    pub pending: bool,
}

/// SQLite needs a single rewrite to add its pointer map when auto-vacuum is
/// first enabled. Never run a full VACUUM on ingest. Hosts call policy changes
/// on a blocking executor.
pub(crate) async fn prepare(connection: &mut SqliteConnection) -> Result<(), LogStoreError> {
    let mode: i64 = sqlx::query_scalar("PRAGMA auto_vacuum")
        .fetch_one(&mut *connection)
        .await?;
    if mode != 2 {
        sqlx::raw_sql("PRAGMA auto_vacuum=INCREMENTAL; VACUUM;")
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

pub(crate) async fn maintain(
    connection: &mut SqliteConnection,
    policy: LogRetentionPolicy,
    now: chrono::NaiveDateTime,
) -> Result<RetentionOutcome, LogStoreError> {
    let mut evicted = Vec::new();
    if !policy.is_enabled() {
        return Ok(RetentionOutcome {
            evicted,
            pending: false,
        });
    }
    let mut transaction = connection.begin().await?;
    if let Some(days) = policy.days {
        let cutoff = now
            .checked_sub_signed(chrono::Duration::days(i64::from(days)))
            .ok_or(LogStoreError::InvalidRetention)?
            .format("%Y-%m-%dT%H:%M:%S%.3f")
            .to_string();
        let sequences: Vec<i64> = sqlx::query_scalar(
            "DELETE FROM logs WHERE sequence IN (SELECT sequence FROM logs WHERE timestamp < ? ORDER BY timestamp, sequence LIMIT ?) RETURNING sequence",
        )
        .bind(cutoff)
        .bind(DELETE_BUDGET as i64)
        .fetch_all(&mut *transaction)
        .await?;
        evicted.extend(sequences.into_iter().map(|sequence| sequence as u64));
    }
    if let Some(mib) = policy.storage_mib {
        let target = u64::from(mib) * MIB;
        while evicted.len() < DELETE_BUDGET && allocated_bytes(&mut transaction).await? > target {
            let budget = SIZE_DELETE_BATCH.min(DELETE_BUDGET - evicted.len());
            let sequences: Vec<i64> = sqlx::query_scalar(
                "DELETE FROM logs WHERE sequence IN (SELECT sequence FROM logs ORDER BY sequence LIMIT ?) RETURNING sequence",
            )
            .bind(budget as i64)
            .fetch_all(&mut *transaction)
            .await?;
            if sequences.is_empty() {
                break;
            }
            evicted.extend(sequences.into_iter().map(|sequence| sequence as u64));
        }
    }
    if !evicted.is_empty() {
        sqlx::query(
            "INSERT OR REPLACE INTO tracing_meta(key,value) VALUES ('retention_evicted','1')",
        )
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    // One bounded page reclamation step; do not VACUUM the full history per batch.
    sqlx::query("PRAGMA incremental_vacuum(256)")
        .execute(&mut *connection)
        .await?;
    let free_pages: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(&mut *connection)
        .await?;
    // A reader holding a WAL snapshot defers truncation without stalling ingest.
    // Restore the normal write timeout even if checkpointing fails.
    sqlx::query("PRAGMA busy_timeout=0")
        .execute(&mut *connection)
        .await?;
    let checkpoint = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(&mut *connection)
        .await;
    sqlx::query("PRAGMA busy_timeout=2000")
        .execute(&mut *connection)
        .await?;
    let checkpoint_busy = checkpoint?.try_get::<i64, _>(0)? != 0;
    let pending = evicted.len() == DELETE_BUDGET || free_pages > 0 || checkpoint_busy;
    evicted.sort_unstable();
    Ok(RetentionOutcome { evicted, pending })
}

async fn allocated_bytes(connection: &mut SqliteConnection) -> Result<u64, LogStoreError> {
    let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(&mut *connection)
        .await?;
    let free: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(&mut *connection)
        .await?;
    let size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(&mut *connection)
        .await?;
    Ok((pages.saturating_sub(free) as u64).saturating_mul(size as u64))
}
