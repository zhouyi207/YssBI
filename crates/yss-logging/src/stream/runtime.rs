use crate::LogRetentionPolicy;
use crate::collector::{CapturedLogSink, OutputHandle};
use crate::store::{LogPage, LogQuery, LogStatistics, LogStoreError};
use std::{path::PathBuf, sync::Arc};

use super::dispatcher::{LogDispatcherGuard, LogDispatcherStartError, LogHub, LogsUnavailable};
use super::dto::{LogBatchDto, LogSubscriptionDto};
use super::rust_projection::log_record_sink;

/// Persists sanitized logs and owns their recent snapshot and live subscribers.
#[derive(Clone)]
pub struct LogRuntime {
    hub: LogHub,
    _dispatcher_guard: Arc<LogDispatcherGuard>,
}

impl LogRuntime {
    pub fn open(path: PathBuf) -> Result<Self, LogInitializationError> {
        Self::open_with_retention(path, LogRetentionPolicy::default())
    }

    /// Opens persistent history and applies the first bounded retention pass
    /// before exposing a snapshot. Remaining work runs on the dispatcher.
    pub fn open_with_retention(
        path: PathBuf,
        policy: LogRetentionPolicy,
    ) -> Result<Self, LogInitializationError> {
        let (hub, dispatcher_guard) = LogHub::start_production(path, policy)?;
        Ok(Self {
            hub,
            _dispatcher_guard: Arc::new(dispatcher_guard),
        })
    }

    pub fn initialize() -> Result<Self, LogInitializationError> {
        let (hub, guard) = LogHub::start_memory()?;
        Ok(Self {
            hub,
            _dispatcher_guard: Arc::new(guard),
        })
    }

    pub fn query(&self, query: LogQuery) -> Result<LogPage, LogStoreError> {
        self.hub.query(query)
    }
    pub fn statistics(&self) -> Result<LogStatistics, LogStoreError> {
        self.hub.statistics()
    }

    /// Changes logging-owned retention without changing collection or display
    /// levels. Call from a blocking executor: SQLite may need a one-time
    /// auto-vacuum conversion. Errors include maintenance failures;
    /// subsequent asynchronous failures terminate subscribers visibly.
    pub fn set_retention(&self, policy: LogRetentionPolicy) -> Result<(), LogStoreError> {
        self.hub.set_retention(policy)
    }
    pub fn shutdown(&self) {
        self._dispatcher_guard.shutdown();
    }

    pub(crate) fn rust_log_sink(&self, console: Option<OutputHandle>) -> CapturedLogSink {
        log_record_sink(self.hub.clone(), console)
    }

    /// Subscribes a platform-neutral bounded batch sink and returns the
    /// current recent snapshot at the same ordered stream boundary.
    pub fn subscribe_batches(
        &self,
        on_records: impl Fn(LogBatchDto) -> bool + Send + 'static,
    ) -> Result<LogSubscriptionDto, LogsUnavailable> {
        self.hub.subscribe(on_records)
    }

    pub fn unsubscribe(&self, subscription_id: String) -> Result<(), LogsUnavailable> {
        self.hub.unsubscribe(subscription_id)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("failed to initialize logs runtime")]
pub struct LogInitializationError {
    #[source]
    source: LogDispatcherStartError,
}

impl From<LogDispatcherStartError> for LogInitializationError {
    fn from(source: LogDispatcherStartError) -> Self {
        Self { source }
    }
}
