use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type LogFields = BTreeMap<String, Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<&tracing::Level> for LogLevel {
    fn from(level: &tracing::Level) -> Self {
        match *level {
            tracing::Level::TRACE => Self::Trace,
            tracing::Level::DEBUG => Self::Debug,
            tracing::Level::INFO => Self::Info,
            tracing::Level::WARN => Self::Warn,
            tracing::Level::ERROR => Self::Error,
        }
    }
}

impl LogLevel {
    pub(crate) const fn enabled_by_default(self) -> bool {
        matches!(self, Self::Info | Self::Warn | Self::Error)
    }
}

/// Owned, size-limited event data awaiting processing by the dispatcher.
pub(crate) struct CapturedLog {
    pub timestamp: chrono::NaiveDateTime,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub fields: LogFields,
}

impl CapturedLog {
    pub(crate) fn into_sanitized_record(self) -> LogRecord {
        LogRecord {
            timestamp: self.timestamp.format("%Y-%m-%dT%H:%M:%S%.3f").to_string(),
            level: self.level,
            target: super::sanitize_target(&self.target),
            message: super::sanitize_message(&self.message),
            fields: super::sanitize_fields(self.fields),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub timestamp: String,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub fields: LogFields,
}

pub(crate) type CapturedLogSink = Arc<dyn Fn(CapturedLog) + Send + Sync + 'static>;
