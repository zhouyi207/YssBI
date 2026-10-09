//! Immutable display and lazy search text for one already-sanitized log record.
use gpui::SharedString;
use std::cell::OnceCell;
use yss_logging::{LogDomain, LogLevel, LogOrigin, LogRecordDto};

pub(super) struct LogEntry {
    pub record: LogRecordDto,
    pub key: SharedString,
    pub time: SharedString,
    pub message: SharedString,
    pub preview: SharedString,
    search: OnceCell<Vec<String>>,
}

impl LogEntry {
    pub fn new(record: LogRecordDto) -> Self {
        let message: SharedString = record.message.clone().into();
        let preview = if record.message.contains(['\n', '\r', '\t']) {
            record.message.replace(['\n', '\r', '\t'], " ").into()
        } else {
            message.clone()
        };
        Self {
            key: format!("{}:{}", record.stream_id, record.sequence).into(),
            time: record
                .timestamp
                .get(11..23)
                .unwrap_or(&record.timestamp)
                .to_owned()
                .into(),
            message,
            preview,
            record,
            search: OnceCell::new(),
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || self
                .search
                .get_or_init(|| {
                    let record = &self.record;
                    vec![
                        record.message.to_lowercase(),
                        record.source.as_deref().unwrap_or_default().to_lowercase(),
                        domain_name(record.domain).into(),
                        match record.origin {
                            LogOrigin::Rust => "rust",
                            LogOrigin::Frontend => "frontend",
                        }
                        .into(),
                        record.target.to_lowercase(),
                        record.event.as_deref().unwrap_or_default().to_lowercase(),
                        serde_json::to_string(&record.fields)
                            .unwrap_or_default()
                            .to_lowercase(),
                    ]
                })
                .iter()
                .any(|field| field.contains(query))
    }
}

pub(super) fn level_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN",
        LogLevel::Info => "INFO",
        LogLevel::Debug => "DEBUG",
        LogLevel::Trace => "TRACE",
    }
}

fn domain_name(domain: LogDomain) -> &'static str {
    match domain {
        LogDomain::Application => "application",
        LogDomain::Execution => "execution",
        LogDomain::System => "system",
        LogDomain::Graph => "graph",
        LogDomain::Data => "data",
        LogDomain::Ui => "ui",
    }
}

pub(super) fn domain_key(domain: Option<LogDomain>) -> &'static str {
    match domain {
        None => "log.domains.all",
        Some(LogDomain::Application) => "log.domains.application",
        Some(LogDomain::Execution) => "log.domains.execution",
        Some(LogDomain::System) => "log.domains.system",
        Some(LogDomain::Graph) => "log.domains.graph",
        Some(LogDomain::Data) => "log.domains.data",
        Some(LogDomain::Ui) => "log.domains.ui",
    }
}
