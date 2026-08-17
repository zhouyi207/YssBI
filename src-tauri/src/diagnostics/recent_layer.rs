use std::collections::BTreeMap;
use std::fmt::{self, Write as _};

use serde_json::{Number, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use super::dispatcher::{DiagnosticsHub, PendingDiagnostic};
use super::dto::{DiagnosticDomain, DiagnosticLevel, DiagnosticOrigin};
use super::sanitizer::{
    MAX_EVENT_BYTES, MAX_FIELD_STRING_BYTES, MAX_MESSAGE_BYTES, MAX_SOURCE_BYTES, MAX_TARGET_BYTES,
    redacted_json_value, sanitize_event, sanitize_field_string, sanitize_message, sanitize_source,
    sanitize_target, should_redact_field,
};

const DOMAIN_FIELD: &str = "diagnostic_domain";
const EVENT_FIELD: &str = "diagnostic_event";
const SOURCE_FIELD: &str = "diagnostic_source";
const TARGET_FIELD: &str = "diagnostic_target";
const SKIP_RECENT_FIELD: &str = "diagnostic_skip_recent";

pub(crate) struct RecentDiagnosticsLayer {
    hub: DiagnosticsHub,
}

impl RecentDiagnosticsLayer {
    pub(crate) fn new(hub: DiagnosticsHub) -> Self {
        Self { hub }
    }
}

impl<S> Layer<S> for RecentDiagnosticsLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = DiagnosticVisitor::default();
        event.record(&mut visitor);
        if visitor.skip_recent {
            return;
        }

        let domain = visitor
            .domain
            .unwrap_or_else(|| infer_domain(metadata.target()));
        let pending = PendingDiagnostic {
            timestamp: super::rfc3339_now(),
            level: DiagnosticLevel::from(metadata.level()),
            origin: DiagnosticOrigin::Rust,
            domain,
            target: visitor
                .target
                .unwrap_or_else(|| sanitize_target(metadata.target())),
            event: visitor.event,
            message: visitor
                .message
                .unwrap_or_else(|| sanitize_message(metadata.name())),
            source: visitor.source,
            fields: visitor.fields,
        };
        let _ = self.hub.publish(vec![pending]);
    }
}

#[derive(Default)]
struct DiagnosticVisitor {
    domain: Option<DiagnosticDomain>,
    target: Option<String>,
    event: Option<String>,
    message: Option<String>,
    source: Option<String>,
    skip_recent: bool,
    fields: BTreeMap<String, Value>,
}

impl DiagnosticVisitor {
    fn record_string(&mut self, field: &Field, value: &str) {
        match field.name() {
            DOMAIN_FIELD => self.domain = DiagnosticDomain::parse(value),
            TARGET_FIELD => self.target = Some(sanitize_target(value)),
            EVENT_FIELD => self.event = Some(sanitize_event(value)),
            SOURCE_FIELD => self.source = Some(sanitize_source(value)),
            "message" => self.message = Some(sanitize_message(value)),
            SKIP_RECENT_FIELD => {}
            name => {
                let value = if should_redact_field(name) {
                    redacted_json_value()
                } else {
                    Value::String(sanitize_field_string(value))
                };
                self.fields.insert(name.to_owned(), value);
            }
        }
    }

    fn record_value(&mut self, field: &Field, value: Value) {
        if field.name() == SKIP_RECENT_FIELD {
            if let Value::Bool(skip_recent) = value {
                self.skip_recent = skip_recent;
            }
            return;
        }
        let value = if should_redact_field(field.name()) {
            redacted_json_value()
        } else {
            value
        };
        self.fields.insert(field.name().to_owned(), value);
    }

    fn formatted_limit(field: &Field) -> usize {
        match field.name() {
            TARGET_FIELD => MAX_TARGET_BYTES,
            EVENT_FIELD => MAX_EVENT_BYTES,
            SOURCE_FIELD => MAX_SOURCE_BYTES,
            "message" => MAX_MESSAGE_BYTES,
            _ => MAX_FIELD_STRING_BYTES,
        }
    }
}

impl Visit for DiagnosticVisitor {
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
        self.record_string(field, value);
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        if should_redact_field(field.name()) {
            self.record_value(field, redacted_json_value());
            return;
        }
        let value = format_bounded(Self::formatted_limit(field), |writer| {
            write!(writer, "{value}")
        });
        self.record_string(field, &value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if should_redact_field(field.name()) {
            self.record_value(field, redacted_json_value());
            return;
        }
        let value = format_bounded(Self::formatted_limit(field), |writer| {
            write!(writer, "{value:?}")
        });
        self.record_string(field, &value);
    }
}

struct BoundedFormatter {
    value: String,
    prefix_limit: usize,
    truncated: bool,
}

impl BoundedFormatter {
    fn new(limit: usize) -> Self {
        let prefix_limit = limit.saturating_sub("…[truncated]".len());
        Self {
            value: String::with_capacity(limit.min(1024)),
            prefix_limit,
            truncated: false,
        }
    }

    fn finish(mut self) -> String {
        if self.truncated {
            self.value.push_str("…[truncated]");
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

fn infer_domain(target: &str) -> DiagnosticDomain {
    if target.contains("runtime")
        || target.contains("execution")
        || target.contains("julia")
        || target.contains("bayes")
    {
        DiagnosticDomain::Execution
    } else if target.contains("database")
        || target.contains("dataframe")
        || target.contains("tabular")
        || target.contains("variable")
    {
        DiagnosticDomain::Data
    } else if target.contains("graph") || target.contains("node_system") {
        DiagnosticDomain::Graph
    } else if target.contains("window") || target.contains("frontend") || target.contains("::ui") {
        DiagnosticDomain::Ui
    } else if target.contains("system") {
        DiagnosticDomain::System
    } else {
        DiagnosticDomain::Application
    }
}
