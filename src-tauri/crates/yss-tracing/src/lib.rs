//! Structured, bounded logging infrastructure for YssBI.
//!
//! This platform-neutral crate owns collection, filtering, sanitization and console output.
//! SQLite persistence and Tauri transport belong to `tauri-plugin-tracing`. Diagnostics remain a separate
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
pub use runtime::{LoggingInitializationError, LoggingRuntime};
pub use sanitizer::{
    REDACTED_VALUE, sanitize_event, sanitize_fields, sanitize_message, sanitize_source,
    sanitize_target,
};
