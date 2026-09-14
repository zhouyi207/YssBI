//! Plugin-owned tracing collection, filtering, sanitization and console output.

mod layer;
mod limits;
mod record;
mod runtime;
mod sanitizer;

pub(crate) use layer::LogLayer;
pub use limits::LogLimits;
pub(crate) use record::{CapturedLog, CapturedLogSink};
pub use record::{LogFields, LogLevel, LogRecord};
pub(crate) use runtime::LoggingRuntime;
pub(crate) use runtime::OutputHandle;
#[cfg(test)]
pub(crate) use runtime::spawn_output;
pub use sanitizer::{
    sanitize_event, sanitize_fields, sanitize_message, sanitize_source, sanitize_target,
};
