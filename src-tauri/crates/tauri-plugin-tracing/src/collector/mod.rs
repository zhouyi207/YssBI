//! Plugin-owned tracing collection, filtering, sanitization and console output.

mod layer;
mod limits;
mod record;
mod runtime;
mod sanitizer;

pub use layer::LogLayer;
pub use limits::LogLimits;
pub use record::{LogFields, LogLevel, LogRecord, LogRecordSink};
pub use runtime::LoggingRuntime;
pub use sanitizer::{
    sanitize_event, sanitize_fields, sanitize_message, sanitize_source, sanitize_target,
};
