use crate::collector::{LoggingInitializationError, LoggingRuntime};
use crate::{LOG_DATABASE_NAME, LogInitializationError, LogRuntime};
use std::path::PathBuf;

/// Owns the process subscriber and drains its dispatcher before console shutdown.
pub struct LogCollection {
    logs: Option<LogRuntime>,
    runtime: LogRuntime,
    _logging: LoggingRuntime,
}

impl LogCollection {
    pub fn initialize(
        directory: Option<PathBuf>,
    ) -> Result<Self, LogCollectionInitializationError> {
        let logs = directory
            .and_then(|directory| LogRuntime::open(directory.join(LOG_DATABASE_NAME)).ok());
        let runtime = match &logs {
            Some(logs) => logs.clone(),
            None => LogRuntime::initialize()?,
        };
        let logging = LoggingRuntime::initialize(|console| runtime.rust_log_sink(Some(console)))?;
        if logs.is_none() {
            tracing::error!(target:"tauri_plugin_tracing",log_domain="system",log_event="logHistoryUnavailable",code="logs_unavailable","Log history storage is unavailable; console logging remains enabled");
        }
        Ok(Self {
            logs,
            runtime,
            _logging: logging,
        })
    }

    pub fn logs(&self) -> Option<&LogRuntime> {
        self.logs.as_ref()
    }

    pub fn shutdown(&self) {
        self.runtime.shutdown();
    }
}

impl Drop for LogCollection {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LogCollectionInitializationError {
    #[error(transparent)]
    Dispatcher(#[from] LogInitializationError),
    #[error(transparent)]
    Collector(#[from] LoggingInitializationError),
}
