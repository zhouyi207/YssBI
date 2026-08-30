//! Structured, bounded logging infrastructure for YssBI.
//!
//! This crate owns tracing collection, filtering, sanitization, console output,
//! and rolling JSONL persistence. Application diagnostics remain a separate
//! projection and may consume sanitized [`LogRecord`] values through a
//! [`LogRecordSink`].

mod layer;
mod limits;
mod record;
mod runtime;
mod sanitizer;

pub use layer::LogLayer;
pub use limits::LogLimits;
pub use record::{LogFields, LogLevel, LogRecord, LogRecordSink};
pub use runtime::{LOG_FILE_NAME, LoggingInitializationError, LoggingRuntime};
pub use sanitizer::{
    REDACTED_VALUE, sanitize_event, sanitize_fields, sanitize_message, sanitize_source,
    sanitize_target,
};
