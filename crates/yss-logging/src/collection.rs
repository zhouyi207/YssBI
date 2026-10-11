use crate::collector::{LoggingInitializationError, LoggingRuntime};
use crate::{LOG_DATABASE_NAME, LogInitializationError, LogRetentionPolicy, LogRuntime};
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
        Self::initialize_with_retention(directory, LogRetentionPolicy::default())
    }

    /// Applies retention before history becomes available to the host.
    pub fn initialize_with_retention(
        directory: Option<PathBuf>,
        policy: LogRetentionPolicy,
    ) -> Result<Self, LogCollectionInitializationError> {
        let logs = directory
            .map(|directory| {
                LogRuntime::open_with_retention(directory.join(LOG_DATABASE_NAME), policy)
            })
            .transpose()?;
        let runtime = match &logs {
            Some(logs) => logs.clone(),
            None => LogRuntime::initialize()?,
        };
        let logging = LoggingRuntime::initialize(|console| runtime.rust_log_sink(Some(console)))?;
        if logs.is_none() {
            tracing::error!(target:"yss_logging",log_domain="system",log_event="logHistoryUnavailable",code="logs_unavailable","Log history storage is unavailable; console logging remains enabled");
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
