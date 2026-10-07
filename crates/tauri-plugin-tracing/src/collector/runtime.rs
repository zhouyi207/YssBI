use std::io::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::SubscriberExt;

use crate::collector::{CapturedLogSink, LogLayer, LogRecord};

const OUTPUT_QUEUE_CAPACITY: usize = 1_024;
const OUTPUT_SHUTDOWN_WAIT: Duration = Duration::from_millis(250);
const OUTPUT_SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Owns the bounded console worker installed by the process-wide logging layer.
pub(crate) struct LoggingRuntime {
    _console_guard: OutputWorkerGuard,
}

impl LoggingRuntime {
    /// Installs one subscriber for all crates and queues captures to the log dispatcher.
    pub(crate) fn initialize(
        connect: impl FnOnce(OutputHandle) -> CapturedLogSink,
    ) -> Result<Self, LoggingInitializationError> {
        let (console, console_guard) = spawn_output("console", create_console_sink())
            .map_err(LoggingInitializationError::ConsoleOutput)?;
        let rust_log = std::env::var("RUST_LOG").ok();
        let filter = logging_filter(rust_log.as_deref());
        let subscriber = tracing_subscriber::registry()
            .with(filter.targets)
            .with(LogLayer::new(connect(console)));
        tracing::subscriber::set_global_default(subscriber)
            .map_err(LoggingInitializationError::TracingSubscriber)?;
        if let Err(error) = tracing_log::LogTracer::init() {
            tracing::warn!(
                target: "tauri_plugin_tracing",
                log_domain = "system",
                log_event = "logTracingBridgeUnavailable",
                error = %error,
                "Failed to install log-to-tracing bridge"
            );
        }
        if let Some(error) = filter.parse_error {
            tracing::warn!(
                target: "tauri_plugin_tracing",
                log_domain = "system",
                log_event = "rustLogFilterInvalid",
                error = %error,
                "Invalid RUST_LOG; using INFO as the minimum log level"
            );
        }
        Ok(Self {
            _console_guard: console_guard,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoggingInitializationError {
    #[error("failed to start console logging output worker")]
    ConsoleOutput(#[source] io::Error),
    #[error("failed to install the global logging subscriber")]
    TracingSubscriber(#[source] tracing::subscriber::SetGlobalDefaultError),
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

pub(crate) struct OutputWorkerGuard {
    sender: SyncSender<OutputCommand>,
    active: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for OutputWorkerGuard {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        let _ = self.sender.try_send(OutputCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let deadline = std::time::Instant::now() + OUTPUT_SHUTDOWN_WAIT;
            while !worker.is_finished() && std::time::Instant::now() < deadline {
                thread::sleep(OUTPUT_SHUTDOWN_POLL_INTERVAL);
            }
            if worker.is_finished() {
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

pub(crate) fn spawn_output(
    name: &str,
    mut sink: OutputSink,
) -> io::Result<(OutputHandle, OutputWorkerGuard)> {
    let (sender, receiver) = mpsc::sync_channel(OUTPUT_QUEUE_CAPACITY);
    let active = Arc::new(AtomicBool::new(true));
    let worker_active = Arc::clone(&active);
    let worker = thread::Builder::new()
        .name(format!("yssbi-log-{name}"))
        .spawn(move || {
            while worker_active.load(Ordering::Acquire) {
                match receiver.recv() {
                    Ok(OutputCommand::Record(record)) => {
                        let succeeded = catch_unwind(AssertUnwindSafe(|| sink(record.as_ref())))
                            .unwrap_or(false);
                        if !succeeded {
                            worker_active.store(false, Ordering::Release);
                        }
                    }
                    Ok(OutputCommand::Shutdown) | Err(_) => break,
                }
            }
            worker_active.store(false, Ordering::Release);
        })?;
    let handle = OutputHandle {
        sender: sender.clone(),
        active: Arc::clone(&active),
    };
    let guard = OutputWorkerGuard {
        sender,
        active,
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

fn logging_filter(rust_log: Option<&str>) -> LoggingFilter {
    let defaults = || Targets::new().with_default(LevelFilter::INFO);
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
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;

    use super::*;
    use crate::collector::LogLevel;

    #[test]
    fn default_and_invalid_filters_collect_info_and_above_from_every_crate() {
        for directives in [None, Some(""), Some("dependency=invalid")] {
            let defaults = logging_filter(directives).targets;
            assert!(!defaults.would_enable("any_crate::worker", &tracing::Level::TRACE));
            assert!(!defaults.would_enable("any_crate::worker", &tracing::Level::DEBUG));
            assert!(defaults.would_enable("any_crate::worker", &tracing::Level::INFO));
            assert!(defaults.would_enable("dependency", &tracing::Level::WARN));
            assert!(defaults.would_enable("dependency", &tracing::Level::ERROR));
            let captured = Arc::new(AtomicUsize::new(0));
            let sink_capture = captured.clone();
            let subscriber = tracing_subscriber::registry()
                .with(defaults)
                .with(LogLayer::new(Arc::new(move |_| {
                    sink_capture.fetch_add(1, Ordering::Relaxed);
                })));
            struct MustNotFormat;
            impl std::fmt::Debug for MustNotFormat {
                fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    panic!("disabled event was formatted");
                }
            }
            tracing::subscriber::with_default(subscriber, || {
                tracing::debug!(value = ?MustNotFormat, "disabled debug");
                tracing::trace!(value = ?MustNotFormat, "disabled trace");
            });
            assert_eq!(captured.load(Ordering::Relaxed), 0);
        }
        let explicit = logging_filter(Some("my_crate=debug")).targets;
        assert!(explicit.would_enable("my_crate::collector::worker", &tracing::Level::DEBUG));
        assert!(!explicit.would_enable("other", &tracing::Level::INFO));
    }

    #[test]
    fn shutdown_releases_an_idle_sink_while_a_producer_handle_is_retained() {
        let (sink_lifetime, released) = mpsc::channel::<()>();
        let sink: OutputSink = Box::new(move |_| {
            let _keep_alive = &sink_lifetime;
            true
        });
        let (_output, guard) = spawn_output("idle-test", sink).unwrap();

        drop(guard);

        assert_eq!(
            released.recv_timeout(Duration::from_secs(1)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
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
            timestamp: "2026-01-01".into(),
            level: LogLevel::Info,
            target: "yssbi::test".into(),
            message: "hello".into(),
            fields: BTreeMap::new(),
        }
    }
}
