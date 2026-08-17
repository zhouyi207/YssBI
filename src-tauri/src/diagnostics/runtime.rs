use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use tauri::ipc::Channel;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::SubscriberExt;

use super::dispatcher::{
    DiagnosticsDispatcherGuard, DiagnosticsHub, DiagnosticsUnavailable, PendingDiagnostic,
    RecordSink,
};
use super::dto::{
    DiagnosticBatchDto, DiagnosticOrigin, DiagnosticRecordDto, DiagnosticSubscriptionDto,
    FrontendDiagnosticEntryDto,
};
use super::recent_layer::RecentDiagnosticsLayer;
use super::validation::{
    FrontendDiagnosticValidationError, ValidatedFrontendDiagnostic, validate_frontend_batch,
};

const LOG_FILE_NAME: &str = "diagnostics.jsonl";
const LOG_ROTATION_BYTES: usize = 10 * 1024 * 1024;
const LOG_ROTATION_FILES: usize = 5;

pub struct DiagnosticsRuntime {
    hub: DiagnosticsHub,
    _dispatcher_guard: DiagnosticsDispatcherGuard,
}

impl DiagnosticsRuntime {
    pub fn initialize(log_dir: Option<PathBuf>) -> Self {
        let mut record_sinks = vec![create_console_record_sink()];
        if let Some(file_sink) = create_file_record_sink(log_dir.as_deref()) {
            record_sinks.push(file_sink);
        }
        let (hub, dispatcher_guard) = DiagnosticsHub::start_with_record_sinks(record_sinks);
        install_tracing(hub.clone());
        Self {
            hub,
            _dispatcher_guard: dispatcher_guard,
        }
    }

    pub(crate) fn submit_frontend(
        &self,
        entries: Vec<FrontendDiagnosticEntryDto>,
    ) -> Result<(), SubmitFrontendDiagnosticsError> {
        let entries = validate_frontend_batch(entries)?;
        let pending = entries.iter().map(frontend_pending).collect();
        self.hub.publish(pending)?;
        Ok(())
    }

    pub(crate) fn subscribe(
        &self,
        on_records: Channel<DiagnosticBatchDto>,
    ) -> Result<DiagnosticSubscriptionDto, DiagnosticsUnavailable> {
        self.hub
            .subscribe(move |batch| on_records.send(batch).is_ok())
    }

    pub(crate) fn unsubscribe(
        &self,
        subscription_id: String,
    ) -> Result<(), DiagnosticsUnavailable> {
        self.hub.unsubscribe(subscription_id)
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SubmitFrontendDiagnosticsError {
    #[error(transparent)]
    Validation(#[from] FrontendDiagnosticValidationError),
    #[error(transparent)]
    Unavailable(#[from] DiagnosticsUnavailable),
}

impl SubmitFrontendDiagnosticsError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "invalid_frontend_diagnostics",
            Self::Unavailable(_) => "diagnostics_unavailable",
        }
    }
}

fn frontend_pending(entry: &ValidatedFrontendDiagnostic) -> PendingDiagnostic {
    PendingDiagnostic {
        timestamp: super::rfc3339_now(),
        level: entry.level,
        origin: DiagnosticOrigin::Frontend,
        domain: entry.domain,
        target: entry.target.clone(),
        event: entry.event.clone(),
        message: entry.message.clone(),
        source: entry.source.clone(),
        fields: entry.fields.clone(),
    }
}

fn create_console_record_sink() -> RecordSink {
    Box::new(|record| {
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        write_json_record(&mut output, record)
    })
}

fn create_file_record_sink(log_dir: Option<&Path>) -> Option<RecordSink> {
    let Some(log_dir) = log_dir else {
        eprintln!("application log directory is unavailable; file diagnostics are disabled");
        return None;
    };
    if let Err(error) = std::fs::create_dir_all(log_dir) {
        eprintln!(
            "failed to create application log directory {}: {error}; file diagnostics are disabled",
            log_dir.display()
        );
        return None;
    }

    let log_path = log_dir.join(LOG_FILE_NAME);
    if let Err(error) = OpenOptions::new().create(true).append(true).open(&log_path) {
        eprintln!(
            "failed to open diagnostics log {}: {error}; file diagnostics are disabled",
            log_path.display()
        );
        return None;
    }

    let writer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        FileRotate::new(
            log_path,
            AppendCount::new(LOG_ROTATION_FILES),
            ContentLimit::BytesSurpassed(LOG_ROTATION_BYTES),
            Compression::None,
            None,
        )
    }));
    match writer {
        Ok(mut writer) => Some(Box::new(move |record| {
            write_json_record(&mut writer, record)
        })),
        Err(_) => {
            eprintln!("failed to start diagnostics file sink; file diagnostics are disabled");
            None
        }
    }
}

fn write_json_record(writer: &mut impl Write, record: &DiagnosticRecordDto) -> bool {
    let Ok(mut line) = serde_json::to_vec(record) else {
        return false;
    };
    line.push(b'\n');
    writer.write_all(&line).is_ok()
}

fn install_tracing(hub: DiagnosticsHub) {
    let rust_log = std::env::var("RUST_LOG").ok();
    let targets = diagnostics_filter(rust_log.as_deref(), cfg!(debug_assertions));
    let subscriber = tracing_subscriber::registry()
        .with(targets)
        .with(RecentDiagnosticsLayer::new(hub));

    if let Err(error) = tracing_log::LogTracer::init() {
        eprintln!("failed to install log-to-tracing bridge: {error}");
    }
    if let Err(error) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("failed to install diagnostics tracing subscriber: {error}");
    }
}

fn diagnostics_filter(rust_log: Option<&str>, debug_build: bool) -> Targets {
    let first_party = if debug_build {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    let defaults = || {
        Targets::new()
            .with_default(LevelFilter::OFF)
            .with_target("yssbi", first_party)
            .with_target("yssbi_lib", first_party)
    };
    let Some(directives) = rust_log.map(str::trim).filter(|value| !value.is_empty()) else {
        return defaults();
    };

    directives.parse::<Targets>().unwrap_or_else(|error| {
        eprintln!("failed to parse RUST_LOG diagnostics target filter: {error}; using defaults");
        defaults()
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};

    use serde_json::json;

    use super::{DiagnosticsRuntime, LOG_FILE_NAME, create_file_record_sink, diagnostics_filter};
    use crate::diagnostics::dispatcher::{DiagnosticsHub, PendingDiagnostic};
    use crate::diagnostics::sanitizer::{MAX_MESSAGE_BYTES, REDACTED_VALUE};
    use crate::diagnostics::{
        DiagnosticDomain, DiagnosticLevel, DiagnosticOrigin, FrontendDiagnosticEntryDto,
    };

    #[test]
    fn file_sink_initialization_failure_is_non_fatal() {
        let directory =
            std::env::temp_dir().join(format!("yssbi-diagnostics-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let file_path = directory.join("not-a-directory");
        std::fs::write(&file_path, b"occupied").unwrap();

        let result = std::panic::catch_unwind(|| create_file_record_sink(Some(&file_path)));
        let writer = result.expect("file sink failures must not unwind into the app");
        assert!(writer.is_none());

        std::fs::remove_file(file_path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn frontend_submission_assigns_rust_stream_metadata_and_frontend_origin() {
        let (hub, dispatcher_guard) = DiagnosticsHub::start();
        let runtime = DiagnosticsRuntime {
            hub,
            _dispatcher_guard: dispatcher_guard,
        };
        runtime
            .submit_frontend(vec![FrontendDiagnosticEntryDto {
                level: DiagnosticLevel::Info,
                domain: DiagnosticDomain::Ui,
                target: "editor.canvas".into(),
                event: Some("selectionChanged".into()),
                message: "Selection changed".into(),
                source: Some("main-window".into()),
                fields: BTreeMap::from([("selectedCount".into(), json!(2))]),
            }])
            .unwrap();

        let subscription = runtime.hub.subscribe(|_| true).unwrap();
        assert_eq!(subscription.latest_sequence, 1);
        let record = &subscription.entries[0];
        assert_eq!(record.sequence, 1);
        assert_eq!(record.origin, DiagnosticOrigin::Frontend);
        assert_eq!(record.domain, DiagnosticDomain::Ui);
        assert_eq!(record.fields["selectedCount"], 2);
        assert!(chrono::DateTime::parse_from_rfc3339(&record.timestamp).is_ok());
        runtime
            .hub
            .unsubscribe(subscription.subscription_id)
            .unwrap();

        let error = runtime.submit_frontend(Vec::new()).unwrap_err();
        assert_eq!(error.code(), "invalid_frontend_diagnostics");
    }

    #[test]
    fn default_filter_is_first_party_and_requires_explicit_rust_log_for_trace() {
        let release = diagnostics_filter(None, false);
        assert!(release.would_enable("yssbi", &tracing::Level::INFO));
        assert!(!release.would_enable("yssbi", &tracing::Level::DEBUG));
        assert!(!release.would_enable("dependency", &tracing::Level::WARN));
        assert!(!release.would_enable("dependency", &tracing::Level::ERROR));

        let debug = diagnostics_filter(None, true);
        assert!(debug.would_enable("yssbi_lib::runtime", &tracing::Level::DEBUG));
        assert!(!debug.would_enable("yssbi_lib", &tracing::Level::TRACE));

        let explicit = diagnostics_filter(Some("yssbi=trace"), false);
        assert!(explicit.would_enable("yssbi::runtime", &tracing::Level::TRACE));
        assert!(!explicit.would_enable("other", &tracing::Level::TRACE));
    }

    #[test]
    fn recent_and_jsonl_file_share_redaction_and_truncation() {
        let directory =
            std::env::temp_dir().join(format!("yssbi-diagnostics-test-{}", uuid::Uuid::new_v4()));
        let file_sink = create_file_record_sink(Some(&directory)).expect("test file sink");
        let (hub, dispatcher_guard) = DiagnosticsHub::start_with_record_sinks(vec![file_sink]);
        let secret = "never-write-this-secret";
        hub.publish(vec![PendingDiagnostic {
            timestamp: super::super::rfc3339_now(),
            level: DiagnosticLevel::Error,
            origin: DiagnosticOrigin::Rust,
            domain: DiagnosticDomain::Data,
            target: "yssbi::database".into(),
            event: Some("queryFailed".into()),
            message: "x".repeat(MAX_MESSAGE_BYTES + 100),
            source: None,
            fields: BTreeMap::from([
                ("database_url".into(), json!(secret)),
                ("nested".into(), json!({ "Authorization": secret })),
            ]),
        }])
        .unwrap();

        let subscription = hub.subscribe(|_| true).unwrap();
        let record = subscription.entries.last().unwrap().clone();
        assert_eq!(record.fields["database_url"], REDACTED_VALUE);
        assert_eq!(record.fields["nested"]["Authorization"], REDACTED_VALUE);
        assert!(record.message.len() <= MAX_MESSAGE_BYTES);
        hub.unsubscribe(subscription.subscription_id).unwrap();

        let log_path = directory.join(LOG_FILE_NAME);
        let started = Instant::now();
        let jsonl = loop {
            if let Ok(contents) = std::fs::read_to_string(&log_path)
                && !contents.is_empty()
            {
                break contents;
            }
            assert!(started.elapsed() < Duration::from_secs(1));
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(!jsonl.contains(secret));
        assert!(jsonl.contains(REDACTED_VALUE));
        let file_record: crate::diagnostics::DiagnosticRecordDto =
            serde_json::from_str(jsonl.lines().last().unwrap()).unwrap();
        assert_eq!(file_record.fields, record.fields);
        assert_eq!(file_record.message, record.message);

        drop(dispatcher_guard);
        let cleanup_started = Instant::now();
        loop {
            match std::fs::remove_dir_all(&directory) {
                Ok(()) => break,
                Err(error) if cleanup_started.elapsed() < Duration::from_secs(1) => {
                    let _ = error;
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("failed to clean diagnostics test directory: {error}"),
            }
        }
    }
}
