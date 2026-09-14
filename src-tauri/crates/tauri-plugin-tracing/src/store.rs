//! SQLite log history. The dispatcher owns this blocking adapter and its connection.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::{ConnectOptions, Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};

use crate::{LogLevel, LogOrigin, LogRecordDto};

pub const LOG_DATABASE_NAME: &str = "logs.sqlite";
pub(crate) const MAX_SAFE_SEQUENCE: u64 = 9_007_199_254_740_991;
const MAX_QUERY_LIMIT: u16 = 500;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct LogQuery {
    pub before_sequence: Option<u64>,
    pub level: Option<LogLevel>,
    pub origin: Option<LogOrigin>,
    pub limit: u16,
}

impl Default for LogQuery {
    fn default() -> Self {
        Self {
            before_sequence: None,
            level: None,
            origin: None,
            limit: 200,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPage {
    pub entries: Vec<LogRecordDto>,
    pub next_before_sequence: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogStatistics {
    pub total: u64,
    pub latest_sequence: u64,
    pub by_level: BTreeMap<String, u64>,
    pub by_origin: BTreeMap<String, u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum LogStoreError {
    #[error("log storage I/O failed")]
    Io(#[from] std::io::Error),
    #[error("log storage database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("stored log data is invalid")]
    Serialization(#[from] serde_json::Error),
    #[error("log query is invalid")]
    InvalidQuery,
    #[error("log sequence is outside the supported range")]
    InvalidSequence,
    #[error("log storage is unavailable")]
    Unavailable,
}

pub(crate) struct StoredSnapshot {
    pub stream_id: String,
    pub latest_sequence: u64,
    pub entries: Vec<LogRecordDto>,
    pub truncated: bool,
}

pub struct LogStore {
    connection: Option<SqliteConnection>,
    runtime: tokio::runtime::Runtime,
    path: PathBuf,
    stream_id: String,
}

impl LogStore {
    /// Open on a dedicated blocking thread, as the plugin dispatcher does.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LogStoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        // The log database must never generate statement logs into its own writer.
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(2))
            .disable_statement_logging();
        let (connection, stream_id) = runtime.block_on(async {
            let mut connection = SqliteConnection::connect_with(&options).await?;
            sqlx::raw_sql(
                "CREATE TABLE IF NOT EXISTS tracing_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE IF NOT EXISTS logs (
                    sequence INTEGER PRIMARY KEY, stream_id TEXT NOT NULL, timestamp TEXT NOT NULL,
                    level TEXT NOT NULL, origin TEXT NOT NULL, domain TEXT NOT NULL,
                    target TEXT NOT NULL, event TEXT, message TEXT NOT NULL, source TEXT,
                    fields TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS logs_level_sequence ON logs(level, sequence);
                 CREATE INDEX IF NOT EXISTS logs_origin_sequence ON logs(origin, sequence);"
            ).execute(&mut connection).await?;
            sqlx::query("INSERT OR IGNORE INTO tracing_meta(key,value) VALUES ('stream_id',?)")
                .bind(uuid::Uuid::new_v4().to_string()).execute(&mut connection).await?;
            let stream_id: String = sqlx::query_scalar("SELECT value FROM tracing_meta WHERE key='stream_id'")
                .fetch_one(&mut connection).await?;
            Ok::<_, sqlx::Error>((connection, stream_id))
        })?;
        Ok(Self {
            connection: Some(connection),
            runtime,
            path,
            stream_id,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn snapshot(&mut self, capacity: usize) -> Result<StoredSnapshot, LogStoreError> {
        let connection = self.connection.as_mut().ok_or(LogStoreError::Unavailable)?;
        let (count, rows) = self.runtime.block_on(async {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
                .fetch_one(&mut *connection)
                .await?;
            let rows = sqlx::query("SELECT * FROM logs ORDER BY sequence DESC LIMIT ?")
                .bind(capacity as i64)
                .fetch_all(&mut *connection)
                .await?;
            Ok::<_, sqlx::Error>((count, rows))
        })?;
        let mut entries = rows
            .into_iter()
            .map(decode_record)
            .collect::<Result<Vec<_>, _>>()?;
        let latest_sequence = entries.first().map_or(0, |entry| entry.sequence);
        entries.reverse();
        Ok(StoredSnapshot {
            stream_id: self.stream_id.clone(),
            latest_sequence,
            entries,
            truncated: count > capacity as i64,
        })
    }

    pub(crate) fn append(&mut self, entries: &[LogRecordDto]) -> Result<(), LogStoreError> {
        if entries.is_empty() {
            return Ok(());
        }
        if entries
            .iter()
            .any(|entry| entry.sequence > MAX_SAFE_SEQUENCE || entry.stream_id != self.stream_id)
        {
            return Err(LogStoreError::InvalidSequence);
        }
        let connection = self.connection.as_mut().ok_or(LogStoreError::Unavailable)?;
        self.runtime.block_on(async {
            let mut transaction = connection.begin().await?;
            for entry in entries {
                sqlx::query("INSERT INTO logs(sequence,stream_id,timestamp,level,origin,domain,target,event,message,source,fields) VALUES (?,?,?,?,?,?,?,?,?,?,?)")
                    .bind(entry.sequence as i64).bind(&entry.stream_id).bind(&entry.timestamp)
                    .bind(enum_text(entry.level)?).bind(enum_text(entry.origin)?).bind(enum_text(entry.domain)?)
                    .bind(&entry.target).bind(&entry.event).bind(&entry.message).bind(&entry.source)
                    .bind(serde_json::to_string(&entry.fields)?)
                    .execute(&mut *transaction).await?;
            }
            transaction.commit().await?;
            Ok(())
        })
    }

    pub fn query(&mut self, query: LogQuery) -> Result<LogPage, LogStoreError> {
        if query.limit == 0
            || query.limit > MAX_QUERY_LIMIT
            || query
                .before_sequence
                .is_some_and(|value| value > MAX_SAFE_SEQUENCE)
        {
            return Err(LogStoreError::InvalidQuery);
        }
        let level = query.level.map(enum_text).transpose()?;
        let origin = query.origin.map(enum_text).transpose()?;
        let before = query.before_sequence.map(|value| value as i64);
        let connection = self.connection.as_mut().ok_or(LogStoreError::Unavailable)?;
        let rows = self.runtime.block_on(
            sqlx::query("SELECT * FROM logs WHERE (? IS NULL OR sequence < ?) AND (? IS NULL OR level = ?) AND (? IS NULL OR origin = ?) ORDER BY sequence DESC LIMIT ?")
                .bind(before).bind(before).bind(&level).bind(&level).bind(&origin).bind(&origin)
                .bind(i64::from(query.limit) + 1).fetch_all(connection)
        )?;
        let mut entries = rows
            .into_iter()
            .map(decode_record)
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = entries.len() > usize::from(query.limit);
        if has_more {
            entries.pop();
        }
        let next_before_sequence = has_more.then(|| entries.last().unwrap().sequence);
        entries.reverse();
        Ok(LogPage {
            entries,
            next_before_sequence,
        })
    }

    pub fn statistics(&mut self) -> Result<LogStatistics, LogStoreError> {
        let connection = self.connection.as_mut().ok_or(LogStoreError::Unavailable)?;
        self.runtime.block_on(async {
            let summary = sqlx::query(
                "SELECT COUNT(*) AS total, COALESCE(MAX(sequence),0) AS latest FROM logs",
            )
            .fetch_one(&mut *connection)
            .await?;
            let levels = sqlx::query("SELECT level, COUNT(*) AS count FROM logs GROUP BY level")
                .fetch_all(&mut *connection)
                .await?;
            let origins = sqlx::query("SELECT origin, COUNT(*) AS count FROM logs GROUP BY origin")
                .fetch_all(&mut *connection)
                .await?;
            Ok(LogStatistics {
                total: summary.try_get::<i64, _>("total")? as u64,
                latest_sequence: summary.try_get::<i64, _>("latest")? as u64,
                by_level: levels
                    .into_iter()
                    .map(|row| {
                        Ok((
                            row.try_get("level")?,
                            row.try_get::<i64, _>("count")? as u64,
                        ))
                    })
                    .collect::<Result<_, sqlx::Error>>()?,
                by_origin: origins
                    .into_iter()
                    .map(|row| {
                        Ok((
                            row.try_get("origin")?,
                            row.try_get::<i64, _>("count")? as u64,
                        ))
                    })
                    .collect::<Result<_, sqlx::Error>>()?,
            })
        })
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
        origin: serde_json::from_value(serde_json::Value::String(row.try_get("origin")?))?,
        domain: serde_json::from_value(serde_json::Value::String(row.try_get("domain")?))?,
        target: row.try_get("target")?,
        event: row.try_get("event")?,
        message: row.try_get("message")?,
        source: row.try_get("source")?,
        fields: serde_json::from_str(&row.try_get::<String, _>("fields")?)?,
    })
}
