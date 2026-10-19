//! SQLite log history. The dispatcher owns this blocking adapter and its connection.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use sqlx::{ConnectOptions, Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};

use crate::LogRecordDto;
use crate::retention::{self, LogRetentionPolicy, RetentionOutcome};

pub const LOG_DATABASE_NAME: &str = "logs.sqlite";
pub(crate) const MAX_SAFE_SEQUENCE: u64 = 9_007_199_254_740_991;

#[derive(Debug, thiserror::Error)]
pub enum LogStoreError {
    #[error("log storage I/O failed")]
    Io(#[from] std::io::Error),
    #[error("log storage database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("stored log data is invalid")]
    Serialization(#[from] serde_json::Error),
    #[error("log sequence is outside the supported range")]
    InvalidSequence,
    #[error("log retention policy is invalid")]
    InvalidRetention,
    #[error("log storage is unavailable")]
    Unavailable,
}

pub(crate) struct StoredSnapshot {
    pub stream_id: String,
    pub latest_sequence: u64,
    pub entries: Vec<LogRecordDto>,
    pub truncated: bool,
}

pub(crate) struct LogStore {
    connection: Option<SqliteConnection>,
    runtime: tokio::runtime::Runtime,
    stream_id: String,
    latest_sequence: u64,
    retention: LogRetentionPolicy,
}

impl LogStore {
    /// Open on the log dispatcher's dedicated blocking thread.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LogStoreError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        // The log database must never generate statement logs into its own writer.
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(2))
            .disable_statement_logging();
        let (connection, stream_id, latest_sequence) = runtime.block_on(async {
            let mut connection = SqliteConnection::connect_with(&options).await?;
            sqlx::raw_sql(
                "CREATE TABLE IF NOT EXISTS tracing_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE IF NOT EXISTS logs (
                    sequence INTEGER PRIMARY KEY, stream_id TEXT NOT NULL, timestamp TEXT NOT NULL,
                    level TEXT NOT NULL, domain TEXT NOT NULL,
                    target TEXT NOT NULL, event TEXT, message TEXT NOT NULL, source TEXT,
                    fields TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS logs_timestamp_sequence ON logs(timestamp, sequence);
                 INSERT OR IGNORE INTO tracing_meta(key,value)
                    SELECT 'latest_sequence', CAST(COALESCE(MAX(sequence),0) AS TEXT) FROM logs;"
            ).execute(&mut connection).await?;
            sqlx::query("INSERT OR IGNORE INTO tracing_meta(key,value) VALUES ('stream_id',?)")
                .bind(uuid::Uuid::new_v4().to_string()).execute(&mut connection).await?;
            let stream_id: String = sqlx::query_scalar("SELECT value FROM tracing_meta WHERE key='stream_id'")
                .fetch_one(&mut connection).await?;
            let latest_sequence: i64 = sqlx::query_scalar("SELECT CAST(value AS INTEGER) FROM tracing_meta WHERE key='latest_sequence'")
                .fetch_one(&mut connection).await?;
            Ok::<_, sqlx::Error>((connection, stream_id, latest_sequence as u64))
        })?;
        Ok(Self {
            connection: Some(connection),
            runtime,
            stream_id,
            latest_sequence,
            retention: LogRetentionPolicy::default(),
        })
    }

    pub(crate) fn set_retention(
        &mut self,
        policy: LogRetentionPolicy,
    ) -> Result<(), LogStoreError> {
        if policy.is_enabled() {
            let connection = self.connection.as_mut().ok_or(LogStoreError::Unavailable)?;
            self.runtime.block_on(retention::prepare(connection))?;
        }
        self.retention = policy;
        Ok(())
    }

    pub(crate) fn maintain(
        &mut self,
        now: chrono::NaiveDateTime,
    ) -> Result<RetentionOutcome, LogStoreError> {
        let connection = self.connection.as_mut().ok_or(LogStoreError::Unavailable)?;
        self.runtime
            .block_on(retention::maintain(connection, self.retention, now))
    }

    pub(crate) fn snapshot(&mut self, capacity: usize) -> Result<StoredSnapshot, LogStoreError> {
        let connection = self.connection.as_mut().ok_or(LogStoreError::Unavailable)?;
        let mut rows = self.runtime.block_on(
            sqlx::query("SELECT * FROM logs ORDER BY sequence DESC LIMIT ?")
                .bind((capacity + 1) as i64)
                .fetch_all(&mut *connection),
        )?;
        let evicted: bool = self.runtime.block_on(
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM tracing_meta WHERE key='retention_evicted')",
            )
            .fetch_one(&mut *connection),
        )?;
        let truncated = rows.len() > capacity || evicted;
        rows.truncate(capacity);
        let mut entries = rows
            .into_iter()
            .map(decode_record)
            .collect::<Result<Vec<_>, _>>()?;
        let latest_sequence = self.latest_sequence;
        entries.reverse();
        Ok(StoredSnapshot {
            stream_id: self.stream_id.clone(),
            latest_sequence,
            entries,
            truncated,
        })
    }

    pub(crate) fn append(&mut self, entries: &[LogRecordDto]) -> Result<(), LogStoreError> {
        if entries.is_empty() {
            return Ok(());
        }
        if entries
            .iter()
            .scan(self.latest_sequence, |previous, entry| {
                let invalid = entry.sequence <= *previous;
                *previous = entry.sequence;
                Some(invalid)
            })
            .any(|invalid| invalid)
            || entries.iter().any(|entry| {
                entry.sequence > MAX_SAFE_SEQUENCE || entry.stream_id != self.stream_id
            })
        {
            return Err(LogStoreError::InvalidSequence);
        }
        let connection = self.connection.as_mut().ok_or(LogStoreError::Unavailable)?;
        self.runtime.block_on(async {
            let mut transaction = connection.begin().await?;
            for entry in entries {
                sqlx::query("INSERT INTO logs(sequence,stream_id,timestamp,level,domain,target,event,message,source,fields) VALUES (?,?,?,?,?,?,?,?,?,?)")
                    .bind(entry.sequence as i64).bind(&entry.stream_id).bind(&entry.timestamp)
                    .bind(enum_text(entry.level)?).bind(enum_text(entry.domain)?)
                    .bind(&entry.target).bind(&entry.event).bind(&entry.message).bind(&entry.source)
                    .bind(serde_json::to_string(&entry.fields)?)
                    .execute(&mut *transaction).await?;
            }
            sqlx::query("UPDATE tracing_meta SET value=? WHERE key='latest_sequence'")
                .bind(entries.last().unwrap().sequence.to_string())
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await?;
            Ok::<_, LogStoreError>(())
        })?;
        self.latest_sequence = entries.last().unwrap().sequence;
        Ok(())
    }
}

impl Drop for LogStore {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let _ = self.runtime.block_on(connection.close());
        }
    }
}

fn enum_text(value: impl Serialize) -> Result<String, serde_json::Error> {
    Ok(serde_json::to_value(value)?.as_str().unwrap().to_owned())
}

fn decode_record(row: sqlx::sqlite::SqliteRow) -> Result<LogRecordDto, LogStoreError> {
    let sequence = u64::try_from(row.try_get::<i64, _>("sequence")?)
        .map_err(|_| LogStoreError::InvalidSequence)?;
    if sequence > MAX_SAFE_SEQUENCE {
        return Err(LogStoreError::InvalidSequence);
    }
    Ok(LogRecordDto {
        stream_id: row.try_get("stream_id")?,
        sequence,
        timestamp: row.try_get("timestamp")?,
        level: serde_json::from_value(serde_json::Value::String(row.try_get("level")?))?,
        domain: serde_json::from_value(serde_json::Value::String(row.try_get("domain")?))?,
        target: row.try_get("target")?,
        event: row.try_get("event")?,
        message: row.try_get("message")?,
        source: row.try_get("source")?,
        fields: serde_json::from_str(&row.try_get::<String, _>("fields")?)?,
    })
}
