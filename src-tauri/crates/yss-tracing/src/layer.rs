use std::fmt::{self, Write as _};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use chrono::{SecondsFormat, Utc};
use serde_json::{Number, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use crate::runtime::OutputHandle;
use crate::{LogFields, LogLevel, LogLimits, LogRecord, LogRecordSink, sanitize_fields};

const MESSAGE_FIELD: &str = "message";
const TRUNCATED_SUFFIX: &str = "…[truncated]";

/// A tracing layer that turns events into bounded, sanitized [`LogRecord`]s.
///
/// Output workers use bounded non-blocking queues. The optional embedding sink
/// is invoked after sanitization and must itself perform only non-blocking work.
pub struct LogLayer {
    outputs: Vec<OutputHandle>,
    record_sink: Option<LogRecordSink>,
}

impl LogLayer {
    /// Creates an embedding layer that forwards sanitized records to `sink`.
    ///
    /// This is useful when another subsystem wants a projection of Rust logs
    /// without taking ownership of logging configuration or files. The sink
    /// must return promptly; panics are isolated from the tracing call site.
    pub fn new(sink: LogRecordSink) -> Self {
        Self {
            outputs: Vec::new(),
            record_sink: Some(sink),
        }
    }

    pub(crate) fn with_outputs(
        outputs: Vec<OutputHandle>,
        record_sink: Option<LogRecordSink>,
    ) -> Self {
        Self {
            outputs,
            record_sink,
        }
    }
}

impl<S> Layer<S> for LogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);

        let record = Arc::new(LogRecord {
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            level: LogLevel::from(metadata.level()),
            target: crate::sanitize_target(metadata.target()),
            message: visitor
                .message
                .unwrap_or_else(|| crate::sanitize_message(metadata.name())),
            fields: sanitize_fields(visitor.fields),
        });

        for output in &self.outputs {
            output.try_enqueue(Arc::clone(&record));
        }
        if let Some(sink) = &self.record_sink {
            let _ = catch_unwind(AssertUnwindSafe(|| sink(record.as_ref())));
        }
    }
}

#[derive(Default)]
struct LogVisitor {
    message: Option<String>,
    fields: LogFields,
}

impl LogVisitor {
    fn record_value(&mut self, field: &Field, value: Value) {
        if field.name() == MESSAGE_FIELD {
            self.message = Some(crate::sanitize_message(&value_to_text(&value)));
        } else {
            self.fields.insert(field.name().to_owned(), value);
        }
    }

    fn formatted_limit(field: &Field) -> usize {
        if field.name() == MESSAGE_FIELD {
            LogLimits::MAX_MESSAGE_BYTES
        } else {
            LogLimits::MAX_FIELD_STRING_BYTES
        }
    }
}

impl Visit for LogVisitor {
    fn record_f64(&mut self, field: &Field, value: f64) {
        let value = Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(value.to_string()));
        self.record_value(field, value);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field, Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field, Value::Number(value.into()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field, Value::Bool(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field, Value::String(value.to_owned()));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        let value = format_bounded(Self::formatted_limit(field), |writer| {
            write!(writer, "{value}")
        });
        self.record_value(field, Value::String(value));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let value = format_bounded(Self::formatted_limit(field), |writer| {
            write!(writer, "{value:?}")
        });
        self.record_value(field, Value::String(value));
    }
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}

struct BoundedFormatter {
    value: String,
    prefix_limit: usize,
    truncated: bool,
}

impl BoundedFormatter {
    fn new(limit: usize) -> Self {
        Self {
            value: String::with_capacity(limit.min(1024)),
            prefix_limit: limit.saturating_sub(TRUNCATED_SUFFIX.len()),
            truncated: false,
        }
    }

    fn finish(mut self) -> String {
        if self.truncated {
            self.value.push_str(TRUNCATED_SUFFIX);
        }
        self.value
    }
}

impl fmt::Write for BoundedFormatter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Ok(());
        }
        let remaining = self.prefix_limit.saturating_sub(self.value.len());
        if value.len() <= remaining {
            self.value.push_str(value);
            return Ok(());
        }

        let mut boundary = remaining;
        while boundary > 0 && !value.is_char_boundary(boundary) {
            boundary -= 1;
        }
        self.value.push_str(&value[..boundary]);
        self.truncated = true;
        Ok(())
    }
}

fn format_bounded(
    limit: usize,
    format: impl FnOnce(&mut BoundedFormatter) -> fmt::Result,
) -> String {
    let mut writer = BoundedFormatter::new(limit);
    let _ = format(&mut writer);
    writer.finish()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    #[test]
    fn emits_sanitized_structured_records() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink_capture = Arc::clone(&captured);
        let sink: LogRecordSink = Arc::new(move |record| {
            sink_capture.lock().unwrap().push(record.clone());
        });
        let subscriber = tracing_subscriber::registry().with(LogLayer::new(sink));

        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(
                target: "yssbi::test",
                password = "secret",
                count = 3_u64,
                "Authorization: Bearer hidden"
            );
        });

        let records = captured.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].target, "yssbi::test");
        assert_eq!(records[0].level, LogLevel::Warn);
        assert_eq!(records[0].fields["password"], crate::REDACTED_VALUE);
        assert_eq!(records[0].fields["count"], 3);
        assert!(!records[0].message.contains("hidden"));
    }

    #[test]
    fn bounds_debug_values_before_allocating_an_unbounded_record() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink_capture = Arc::clone(&captured);
        let sink: LogRecordSink = Arc::new(move |record| {
            sink_capture.lock().unwrap().push(record.clone());
        });
        let subscriber = tracing_subscriber::registry().with(LogLayer::new(sink));
        let oversized = "x".repeat(LogLimits::MAX_FIELD_STRING_BYTES * 2);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(payload = ?oversized, "bounded");
        });

        let records = captured.lock().unwrap();
        assert!(
            records[0].fields["payload"].as_str().unwrap().len()
                <= LogLimits::MAX_FIELD_STRING_BYTES
        );
    }
}
