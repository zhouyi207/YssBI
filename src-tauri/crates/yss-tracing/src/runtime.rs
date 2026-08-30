use std::fs::OpenOptions;
use std::io::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::SubscriberExt;

use crate::{LogLayer, LogRecord, LogRecordSink};

pub const LOG_FILE_NAME: &str = "yssbi.log.jsonl";
const LOG_ROTATION_BYTES: usize = 10 * 1024 * 1024;
const LOG_ROTATION_FILES: usize = 5;
const OUTPUT_QUEUE_CAPACITY: usize = 1_024;
const OUTPUT_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const OUTPUT_SHUTDOWN_WAIT: Duration = Duration::from_millis(250);
const OUTPUT_SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Owns the bounded output workers installed by the process-wide logging layer.
pub struct LoggingRuntime {
    _output_guards: Vec<OutputWorkerGuard>,
}

impl LoggingRuntime {
    /// Installs the process-wide tracing subscriber and optional log projection.
    ///
    /// Console logging is mandatory. File logging is best-effort and emits a
    /// structured warning when its directory or worker cannot be initialized.
    pub fn initialize(
        log_dir: Option<PathBuf>,
        record_sink: Option<LogRecordSink>,
    ) -> Result<Self, LoggingInitializationError> {
        let (console, console_guard) =
            spawn_output("console", create_console_sink()).map_err(|source| {
                LoggingInitializationError::OutputWorker {
                    name: "console".into(),
                    source,
                }
            })?;
        let mut outputs = vec![console];
        let mut guards = vec![console_guard];
        let mut file_warning = None;

        if let Some(log_dir) = log_dir {
            match create_file_sink(&log_dir).and_then(|sink| {
                spawn_output("file", sink).map_err(|source| FileLogSinkError::Worker {
                    path: log_dir.join(LOG_FILE_NAME),
                    source,
                })
            }) {
                Ok((file, file_guard)) => {
                    outputs.push(file);
                    guards.push(file_guard);
                }
                Err(error) => file_warning = Some(error),
            }
        }

        let rust_log = std::env::var("RUST_LOG").ok();
        let filter = logging_filter(rust_log.as_deref(), cfg!(debug_assertions));
        let subscriber = tracing_subscriber::registry()
            .with(filter.targets)
            .with(LogLayer::with_outputs(outputs, record_sink));
        tracing::subscriber::set_global_default(subscriber)
            .map_err(LoggingInitializationError::TracingSubscriber)?;

        if let Err(error) = tracing_log::LogTracer::init() {
            tracing::warn!(
                target: "yssbi::logging",
                diagnostic_domain = "system",
                diagnostic_event = "logTracingBridgeUnavailable",
                error = %error,
                "Failed to install log-to-tracing bridge"
            );
        }
        if let Some(error) = filter.parse_error {
            tracing::warn!(
                target: "yssbi::logging",
                diagnostic_domain = "system",
                diagnostic_event = "rustLogFilterInvalid",
                error = %error,
                "Failed to parse RUST_LOG target filter; using defaults"
            );
        }
        if let Some(error) = file_warning {
            tracing::warn!(
                target: "yssbi::logging",
                diagnostic_domain = "system",
                diagnostic_event = "logFileSinkUnavailable",
                failure_stage = error.stage(),
                path = %error.path().display(),
                error = %error,
                "File logging is disabled"
            );
        }

        Ok(Self {
            _output_guards: guards,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoggingInitializationError {
    #[error("failed to start {name} logging output worker")]
    OutputWorker {
        name: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to install the global logging subscriber")]
    TracingSubscriber(#[source] tracing::subscriber::SetGlobalDefaultError),
}

#[derive(Debug, thiserror::Error)]
enum FileLogSinkError {
    #[error("failed to create log directory '{}': {source}", path.display())]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to open log file '{}': {source}", path.display())]
    OpenFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("log file rotation initialization panicked for '{}'", path.display())]
    InitializationPanicked { path: PathBuf },
    #[error("failed to start log file worker for '{}': {source}", path.display())]
    Worker {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl FileLogSinkError {
    const fn stage(&self) -> &'static str {
        match self {
            Self::CreateDirectory { .. } => "create_directory",
            Self::OpenFile { .. } => "open_file",
            Self::InitializationPanicked { .. } => "initialize_rotation",
            Self::Worker { .. } => "start_worker",
        }
    }

    fn path(&self) -> &Path {
        match self {
            Self::CreateDirectory { path, .. }
            | Self::OpenFile { path, .. }
            | Self::InitializationPanicked { path }
            | Self::Worker { path, .. } => path,
        }
    }
}

enum OutputCommand {
    Record(Arc<LogRecord>),
    Shutdown,
}

#[derive(Clone)]
pub(crate) struct OutputHandle {
    sender: SyncSender<OutputCommand>,
    active: Arc<AtomicBool>,
}

impl OutputHandle {
    pub(crate) fn try_enqueue(&self, record: Arc<LogRecord>) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }
        match self.sender.try_send(OutputCommand::Record(record)) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {
                self.active.store(false, Ordering::Release);
            }
        }
    }
}

struct OutputWorkerGuard {
    sender: SyncSender<OutputCommand>,
    active: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for OutputWorkerGuard {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        match self.sender.try_send(OutputCommand::Shutdown) {
            Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
        if let Some(worker) = self.worker.take() {
            let deadline = std::time::Instant::now() + OUTPUT_SHUTDOWN_WAIT;
            while !self.finished.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
                thread::sleep(OUTPUT_SHUTDOWN_POLL_INTERVAL);
            }
            if self.finished.load(Ordering::Acquire) {
                // Sink panics are caught in the worker, so a join failure can only
                // come from an unexpected worker invariant violation. Logging is
                // already being torn down and cannot safely report recursively.
                worker
                    .join()
                    .expect("logging output worker isolates sink panics");
            }
        }
    }
}

type OutputSink = Box<dyn FnMut(&LogRecord) -> bool + Send + 'static>;

fn spawn_output(name: &str, mut sink: OutputSink) -> io::Result<(OutputHandle, OutputWorkerGuard)> {
    let (sender, receiver) = mpsc::sync_channel(OUTPUT_QUEUE_CAPACITY);
    let active = Arc::new(AtomicBool::new(true));
    let worker_active = Arc::clone(&active);
    let finished = Arc::new(AtomicBool::new(false));
    let worker_finished = Arc::clone(&finished);
    let worker = thread::Builder::new()
        .name(format!("yssbi-log-{name}"))
        .spawn(move || {
            while worker_active.load(Ordering::Acquire) {
                match receiver.recv_timeout(OUTPUT_IDLE_POLL_INTERVAL) {
                    Ok(OutputCommand::Record(record)) => {
                        let succeeded = catch_unwind(AssertUnwindSafe(|| sink(record.as_ref())))
                            .unwrap_or(false);
                        if !succeeded {
                            worker_active.store(false, Ordering::Release);
                        }
                    }
                    Ok(OutputCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
            worker_active.store(false, Ordering::Release);
            worker_finished.store(true, Ordering::Release);
        })?;
    let handle = OutputHandle {
        sender: sender.clone(),
        active: Arc::clone(&active),
    };
    let guard = OutputWorkerGuard {
        sender,
        active,
        finished,
        worker: Some(worker),
    };
    Ok((handle, guard))
}

fn create_console_sink() -> OutputSink {
    Box::new(|record| {
        let stdout = io::stdout();
        let mut output = stdout.lock();
        write_json_record(&mut output, record)
    })
}

fn create_file_sink(log_dir: &Path) -> Result<OutputSink, FileLogSinkError> {
    std::fs::create_dir_all(log_dir).map_err(|source| FileLogSinkError::CreateDirectory {
        path: log_dir.to_path_buf(),
        source,
    })?;

    let log_path = log_dir.join(LOG_FILE_NAME);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|source| FileLogSinkError::OpenFile {
            path: log_path.clone(),
            source,
        })?;

    let writer = catch_unwind(AssertUnwindSafe(|| {
        FileRotate::new(
            log_path.clone(),
            AppendCount::new(LOG_ROTATION_FILES),
            ContentLimit::BytesSurpassed(LOG_ROTATION_BYTES),
            Compression::None,
            None,
        )
    }))
    .map_err(|_| FileLogSinkError::InitializationPanicked { path: log_path })?;
    let mut writer = writer;
    Ok(Box::new(move |record| {
        write_json_record(&mut writer, record)
    }))
}

fn write_json_record(writer: &mut impl Write, record: &LogRecord) -> bool {
    let Ok(mut line) = serde_json::to_vec(record) else {
        return false;
    };
    line.push(b'\n');
    writer.write_all(&line).is_ok()
}

struct LoggingFilter {
    targets: Targets,
    parse_error: Option<String>,
}

fn logging_filter(rust_log: Option<&str>, debug_build: bool) -> LoggingFilter {
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
            .with_target("yss_tracing", first_party)
    };
    let Some(directives) = rust_log.map(str::trim).filter(|value| !value.is_empty()) else {
        return LoggingFilter {
            targets: defaults(),
            parse_error: None,
        };
    };

    match directives.parse::<Targets>() {
        Ok(targets) => LoggingFilter {
            targets,
            parse_error: None,
        },
        Err(error) => LoggingFilter {
            targets: defaults(),
            parse_error: Some(error.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::mpsc;

    use super::*;
    use crate::LogLevel;

    #[test]
    fn file_sink_initialization_failure_is_structured() {
        let directory = unique_temp_directory();
        std::fs::create_dir_all(&directory).unwrap();
        let file_path = directory.join("not-a-directory");
        std::fs::write(&file_path, b"occupied").unwrap();

        let error = match create_file_sink(&file_path) {
            Ok(_) => panic!("invalid log directory unexpectedly succeeded"),
            Err(error) => error,
        };
        assert_eq!(error.stage(), "create_directory");

        std::fs::remove_file(file_path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn rolling_file_serializes_log_contract() {
        let directory = unique_temp_directory();
        let mut sink = create_file_sink(&directory).unwrap();
        let record = LogRecord {
            timestamp: "2026-01-01T00:00:00.000Z".into(),
            level: LogLevel::Info,
            target: "yssbi::test".into(),
            message: "hello".into(),
            fields: BTreeMap::from([("count".into(), serde_json::json!(1))]),
        };
        assert!(sink(&record));
        drop(sink);

        let encoded = std::fs::read_to_string(directory.join(LOG_FILE_NAME)).unwrap();
        let decoded: LogRecord = serde_json::from_str(encoded.trim()).unwrap();
        assert_eq!(decoded, record);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn default_filter_is_first_party_and_trace_requires_explicit_rust_log() {
        let release = logging_filter(None, false).targets;
        assert!(release.would_enable("yssbi", &tracing::Level::INFO));
        assert!(!release.would_enable("yssbi", &tracing::Level::DEBUG));
        assert!(!release.would_enable("dependency", &tracing::Level::ERROR));

        let debug = logging_filter(None, true).targets;
        assert!(debug.would_enable("yssbi_lib::runtime", &tracing::Level::DEBUG));
        assert!(!debug.would_enable("yss_tracing", &tracing::Level::TRACE));

        let explicit = logging_filter(Some("yssbi=trace"), false).targets;
        assert!(explicit.would_enable("yssbi::runtime", &tracing::Level::TRACE));
        assert!(!explicit.would_enable("other", &tracing::Level::TRACE));
    }

    #[test]
    fn shutdown_does_not_wait_indefinitely_for_a_blocked_sink() {
        let (started_sender, started_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let (finished_sender, finished_receiver) = mpsc::channel();
        let sink: OutputSink = Box::new(move |_| {
            started_sender.send(()).unwrap();
            let released = release_receiver
                .recv_timeout(Duration::from_secs(5))
                .is_ok();
            finished_sender.send(()).unwrap();
            released
        });
        let (output, guard) = spawn_output("blocked-test", sink).unwrap();
        output.try_enqueue(Arc::new(test_record()));
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        let started = std::time::Instant::now();
        drop(guard);
        assert!(started.elapsed() < Duration::from_secs(1));

        release_sender.send(()).unwrap();
        finished_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
    }

    fn test_record() -> LogRecord {
        LogRecord {
            timestamp: "2026-01-01T00:00:00.000Z".into(),
            level: LogLevel::Info,
            target: "yssbi::test".into(),
            message: "hello".into(),
            fields: BTreeMap::new(),
        }
    }

    fn unique_temp_directory() -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("yssbi-log-test-{}-{nonce}", std::process::id()))
    }
}
