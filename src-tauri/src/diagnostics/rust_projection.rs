use std::sync::Arc;

use serde_json::Value;
use yss_tracing::{
    LogLevel, LogRecord, LogRecordSink, sanitize_event, sanitize_source, sanitize_target,
};

use super::dispatcher::{DiagnosticsHub, PendingDiagnostic};
use super::dto::{DiagnosticDomain, DiagnosticLevel, DiagnosticOrigin};

const DOMAIN_FIELD: &str = "diagnostic_domain";
const EVENT_FIELD: &str = "diagnostic_event";
const SOURCE_FIELD: &str = "diagnostic_source";
const TARGET_FIELD: &str = "diagnostic_target";
const SKIP_RECENT_FIELD: &str = "diagnostic_skip_recent";

pub(crate) fn log_record_sink(hub: DiagnosticsHub) -> LogRecordSink {
    Arc::new(move |record| {
        if let Some(pending) = project_log_record(record) {
            // Diagnostics is an explicitly lossy projection. A rejected
            // projection cannot be logged here without recursively re-entering
            // the logging sink.
            drop(hub.publish(vec![pending]));
        }
    })
}

fn project_log_record(record: &LogRecord) -> Option<PendingDiagnostic> {
    let mut fields = record.fields.clone();
    let skip_recent = take_bool(&mut fields, SKIP_RECENT_FIELD).unwrap_or(false);
    let domain = take_string(&mut fields, DOMAIN_FIELD)
        .as_deref()
        .and_then(DiagnosticDomain::parse)
        .unwrap_or_else(|| infer_domain(&record.target));
    let target = take_string(&mut fields, TARGET_FIELD)
        .map(|value| sanitize_target(&value))
        .unwrap_or_else(|| record.target.clone());
    let event = take_string(&mut fields, EVENT_FIELD).map(|value| sanitize_event(&value));
    let source = take_string(&mut fields, SOURCE_FIELD).map(|value| sanitize_source(&value));
    if skip_recent {
        return None;
    }

    Some(PendingDiagnostic {
        timestamp: record.timestamp.clone(),
        level: DiagnosticLevel::from(record.level),
        origin: DiagnosticOrigin::Rust,
        domain,
        target,
        event,
        message: record.message.clone(),
        source,
        fields,
    })
}

fn take_string(fields: &mut yss_tracing::LogFields, name: &str) -> Option<String> {
    match fields.remove(name) {
        Some(Value::String(value)) => Some(value),
        _ => None,
    }
}

fn take_bool(fields: &mut yss_tracing::LogFields, name: &str) -> Option<bool> {
    match fields.remove(name) {
        Some(Value::Bool(value)) => Some(value),
        _ => None,
    }
}

impl From<LogLevel> for DiagnosticLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Self::Trace,
            LogLevel::Debug => Self::Debug,
            LogLevel::Info => Self::Info,
            LogLevel::Warn => Self::Warn,
            LogLevel::Error => Self::Error,
        }
    }
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
    } else if target.contains("system") || target.contains("logging") {
        DiagnosticDomain::System
    } else {
        DiagnosticDomain::Application
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    fn record(fields: yss_tracing::LogFields) -> LogRecord {
        LogRecord {
            timestamp: "2026-01-01T00:00:00.000Z".into(),
            level: LogLevel::Warn,
            target: "yssbi::database".into(),
            message: "failed".into(),
            fields,
        }
    }

    #[test]
    fn consumes_diagnostic_metadata_without_leaking_it_into_fields() {
        let pending = project_log_record(&record(BTreeMap::from([
            (DOMAIN_FIELD.into(), json!("graph")),
            (EVENT_FIELD.into(), json!("nodeFailed")),
            (SOURCE_FIELD.into(), json!("node-1")),
            (TARGET_FIELD.into(), json!("graph.runner")),
            ("attempt".into(), json!(2)),
        ])))
        .unwrap();

        assert_eq!(pending.domain, DiagnosticDomain::Graph);
        assert_eq!(pending.event.as_deref(), Some("nodeFailed"));
        assert_eq!(pending.source.as_deref(), Some("node-1"));
        assert_eq!(pending.target, "graph.runner");
        assert_eq!(
            pending.fields,
            BTreeMap::from([("attempt".into(), json!(2))])
        );
    }

    #[test]
    fn skip_recent_suppresses_only_the_diagnostic_projection() {
        assert!(
            project_log_record(&record(BTreeMap::from([(
                SKIP_RECENT_FIELD.into(),
                json!(true),
            )])))
            .is_none()
        );
    }
}
