use std::fmt::{self, Write as _};
use std::panic::{AssertUnwindSafe, catch_unwind};

use chrono::Local;
use serde_json::{Number, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use crate::collector::sanitizer::{redacted_json_value, should_redact_field};
use crate::collector::{CapturedLog, CapturedLogSink, LogFields, LogLevel, LogLimits};

const MESSAGE_FIELD: &str = "message";
const TRUNCATED_SUFFIX: &str = "…[truncated]";
const TRUNCATED_FIELD: &str = "_logTruncated";
const TRUNCATION_FIELD_BUDGET: usize = 64;

/// Copies borrowed event data into a bounded record for non-blocking dispatch.
pub(crate) struct LogLayer {
    capture_sink: CapturedLogSink,
}

impl LogLayer {
    pub(crate) fn new(capture_sink: CapturedLogSink) -> Self {
        Self { capture_sink }
    }
}

impl<S> Layer<S> for LogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let timestamp = Local::now().naive_local();
        let metadata = event.metadata();
        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);
        if visitor.truncated {
            visitor
                .fields
                .insert(TRUNCATED_FIELD.into(), Value::Bool(true));
        }
        let record = CapturedLog {
            timestamp,
            level: LogLevel::from(metadata.level()),
            target: bounded_text(metadata.target(), LogLimits::MAX_TARGET_BYTES),
            message: visitor
                .message
                .unwrap_or_else(|| bounded_text(metadata.name(), LogLimits::MAX_MESSAGE_BYTES)),
            fields: visitor.fields,
        };
        let _ = catch_unwind(AssertUnwindSafe(|| (self.capture_sink)(record)));
    }
}

#[derive(Default)]
struct LogVisitor {
    message: Option<String>,
    fields: LogFields,
    field_bytes: usize,
    truncated: bool,
}

impl LogVisitor {
    fn record_with(&mut self, field: &Field, capture: impl FnOnce(usize) -> Value) {
        if field.name() == MESSAGE_FIELD {
            self.message = Some(value_to_text(capture(LogLimits::MAX_MESSAGE_BYTES)));
            return;
        }
        // Reserve space for the truncation marker. Encoded JSON limits are
        // enforced by the dispatcher; this budget bounds capture allocations.
        let remaining = LogLimits::MAX_FIELDS_BYTES
            .saturating_sub(TRUNCATION_FIELD_BUDGET + self.field_bytes + field.name().len());
        if self.fields.len() >= LogLimits::MAX_FIELD_COUNT - 1
            || field.name().len() > LogLimits::MAX_FIELD_KEY_BYTES
            || remaining < 32
        {
            self.truncated = true;
            return;
        }
        let value = if should_redact_field(field.name()) {
            redacted_json_value()
        } else {
            capture(remaining.min(LogLimits::MAX_FIELD_STRING_BYTES))
        };
        self.field_bytes += field.name().len()
            + match &value {
                Value::String(value) => value.len(),
                _ => 32,
            };
        self.fields.insert(field.name().into(), value);
    }
}

impl Visit for LogVisitor {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_with(field, |_| {
            Number::from_f64(value)
                .map(Value::Number)
                .unwrap_or_else(|| Value::String(value.to_string()))
        });
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_with(field, |_| Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_with(field, |_| Value::Number(value.into()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_with(field, |_| Value::Bool(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_with(field, |limit| Value::String(bounded_text(value, limit)));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_with(field, |limit| {
            Value::String(format_bounded(limit, |writer| write!(writer, "{value}")))
        });
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record_with(field, |limit| {
            Value::String(format_bounded(limit, |writer| write!(writer, "{value:?}")))
        });
    }
}

fn value_to_text(value: Value) -> String {
    match value {
        Value::String(value) => value,
        value => value.to_string(),
    }
}

fn bounded_text(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_owned();
    }
    format_bounded(limit, |writer| writer.write_str(value))
}

struct BoundedFormatter {
    value: String,
    limit: usize,
    prefix_limit: usize,
    truncated: bool,
}

impl BoundedFormatter {
    fn new(limit: usize) -> Self {
        Self {
            value: String::with_capacity(limit.min(1024)),
            limit,
            prefix_limit: limit.saturating_sub(TRUNCATED_SUFFIX.len()),
            truncated: false,
        }
    }

    fn finish(mut self) -> String {
        if self.truncated {
            for character in TRUNCATED_SUFFIX.chars() {
                if self.value.len() + character.len_utf8() > self.limit {
                    break;
                }
                self.value.push(character);
            }
        }
        self.value
    }
}

impl fmt::Write for BoundedFormatter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Err(fmt::Error);
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
        Err(fmt::Error)
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    #[test]
    fn emits_sanitized_structured_records() {
        let logs = crate::LogRuntime::initialize().unwrap();
        let subscriber =
            tracing_subscriber::registry().with(LogLayer::new(logs.rust_log_sink(None)));

        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(
                target: "yssbi::test",
                password = "secret",
                count = 3_u64,
                "Authorization: Bearer hidden"
            );
        });

        let snapshot = logs.subscribe_batches(|_| true).unwrap();
        let records = snapshot.entries;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].target, "yssbi::test");
        assert_eq!(records[0].level, LogLevel::Warn);
        assert_eq!(
            records[0].fields["password"],
            crate::collector::sanitizer::REDACTED_VALUE
        );
        assert_eq!(records[0].fields["count"], 3);
        assert!(!records[0].message.contains("hidden"));
    }

    #[test]
    fn bounds_debug_values_before_allocating_an_unbounded_record() {
        struct CountWrites<'a>(&'a AtomicUsize);
        impl fmt::Debug for CountWrites<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for _ in 0..LogLimits::MAX_FIELD_STRING_BYTES * 2 {
                    self.0.fetch_add(1, Ordering::Relaxed);
                    formatter.write_str("x")?;
                }
                Ok(())
            }
        }
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink_capture = Arc::clone(&captured);
        let sink: CapturedLogSink = Arc::new(move |record| {
            sink_capture.lock().unwrap().push(record);
        });
        let subscriber = tracing_subscriber::registry().with(LogLayer::new(sink));
        let writes = AtomicUsize::new(0);
        let oversized = CountWrites(&writes);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(payload = ?oversized, "bounded");
        });

        let records = captured.lock().unwrap();
        assert!(writes.load(Ordering::Relaxed) <= LogLimits::MAX_FIELD_STRING_BYTES);
        assert!(
            records[0].fields["payload"].as_str().unwrap().len()
                <= LogLimits::MAX_FIELD_STRING_BYTES
        );
    }

    #[test]
    fn capture_budget_skips_sensitive_and_excess_fields() {
        struct MustNotFormat;
        impl fmt::Debug for MustNotFormat {
            fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
                panic!("discarded fields must not be formatted");
            }
        }
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink_capture = captured.clone();
        let subscriber =
            tracing_subscriber::registry().with(LogLayer::new(Arc::new(move |record| {
                sink_capture.lock().unwrap().push(record);
            })));
        let large = "界".repeat(LogLimits::MAX_FIELD_STRING_BYTES);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                password = ?MustNotFormat, rows = ?MustNotFormat,
                a = large.as_str(), b = large.as_str(), c = large.as_str(),
                d = large.as_str(), e = large.as_str(), f = large.as_str(),
                g = large.as_str(), h = large.as_str(), excess = ?MustNotFormat,
                "bounded fields"
            );
        });
        let records = captured.lock().unwrap();
        let fields = &records[0].fields;
        assert!(!fields.contains_key("excess"));
        assert_eq!(fields[TRUNCATED_FIELD], true);
        assert_eq!(fields["password"], redacted_json_value());
        let captured_bytes: usize = fields
            .iter()
            .map(|(key, value)| key.len() + value.as_str().map_or(32, str::len))
            .sum();
        assert!(captured_bytes <= LogLimits::MAX_FIELDS_BYTES);
    }
}
