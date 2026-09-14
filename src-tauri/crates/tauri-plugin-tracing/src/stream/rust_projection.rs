use std::sync::Arc;

use serde_json::Value;
use yss_tracing::{LogRecord, LogRecordSink, sanitize_event, sanitize_source, sanitize_target};

use super::dispatcher::{LogHub, PendingLog};
use super::dto::{LogDomain, LogOrigin};

const DOMAIN_FIELD: &str = "diagnostic_domain";
const EVENT_FIELD: &str = "diagnostic_event";
const SOURCE_FIELD: &str = "diagnostic_source";
const TARGET_FIELD: &str = "diagnostic_target";

pub(crate) fn log_record_sink(hub: LogHub) -> LogRecordSink {
    Arc::new(move |record| {
        // Reporting a rejected log here would recursively enter this same sink.
        drop(hub.publish(vec![project_log_record(record)]));
    })
}

fn project_log_record(record: &LogRecord) -> PendingLog {
    let mut fields = record.fields.clone();
    let domain = take_string(&mut fields, DOMAIN_FIELD)
        .as_deref()
        .and_then(LogDomain::parse)
        .unwrap_or_else(|| infer_domain(&record.target));
    let target = take_string(&mut fields, TARGET_FIELD)
        .map(|value| sanitize_target(&value))
        .unwrap_or_else(|| record.target.clone());
    let event = take_string(&mut fields, EVENT_FIELD).map(|value| sanitize_event(&value));
    let source = take_string(&mut fields, SOURCE_FIELD).map(|value| sanitize_source(&value));
    PendingLog {
        timestamp: record.timestamp.clone(),
        level: record.level,
        origin: LogOrigin::Rust,
        domain,
        target,
        event,
        message: record.message.clone(),
        source,
        fields,
    }
}

fn take_string(fields: &mut crate::LogFields, name: &str) -> Option<String> {
    match fields.remove(name) {
        Some(Value::String(value)) => Some(value),
        _ => None,
    }
}

fn infer_domain(target: &str) -> LogDomain {
    if target.contains("runtime")
        || target.contains("execution")
        || target.contains("julia")
        || target.contains("bayes")
    {
        LogDomain::Execution
    } else if target.contains("database")
        || target.contains("dataframe")
        || target.contains("tabular")
        || target.contains("variable")
    {
        LogDomain::Data
    } else if target.contains("graph") || target.contains("node_system") {
        LogDomain::Graph
    } else if target.contains("window") || target.contains("frontend") || target.contains("::ui") {
        LogDomain::Ui
    } else if target.contains("system") || target.contains("logging") {
        LogDomain::System
    } else {
        LogDomain::Application
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::LogLevel;
    use serde_json::json;

    use super::*;

    fn record(fields: crate::LogFields) -> LogRecord {
        LogRecord {
            timestamp: "2026-01-01".into(),
            level: LogLevel::Warn,
            target: "yssbi::database".into(),
            message: "failed".into(),
            fields,
        }
    }

    #[test]
    fn ordinary_crate_events_are_collected_without_diagnostic_annotations() {
        let pending = project_log_record(&record(BTreeMap::from([("count".into(), json!(1))])));
        assert_eq!(pending.domain, LogDomain::Data);
        assert_eq!(pending.message, "failed");
        assert_eq!(pending.fields["count"], 1);
    }
}
