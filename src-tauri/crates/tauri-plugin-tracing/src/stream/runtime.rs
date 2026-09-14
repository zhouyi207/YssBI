use crate::collector::{CapturedLogSink, OutputHandle};
use crate::store::{LogPage, LogQuery, LogStatistics, LogStoreError};
use std::{path::PathBuf, sync::Arc};

use super::dispatcher::{
    LogDispatcherGuard, LogDispatcherStartError, LogHub, LogsUnavailable, PendingLog,
};
use super::dto::{FrontendLogEntryDto, LogBatchDto, LogOrigin, LogSubscriptionDto};
use super::rust_projection::log_record_sink;
use super::validation::{
    FrontendLogValidationError, ValidatedFrontendLog, validate_frontend_batch,
};

/// Persists sanitized logs and owns their recent snapshot and live subscribers.
#[derive(Clone)]
pub struct LogRuntime {
    hub: LogHub,
    _dispatcher_guard: Arc<LogDispatcherGuard>,
}

impl LogRuntime {
    pub fn open(path: PathBuf) -> Result<Self, LogInitializationError> {
        let (hub, dispatcher_guard) = LogHub::start_production(path)?;
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
    pub fn shutdown(&self) {
        self._dispatcher_guard.shutdown();
    }

    pub(crate) fn rust_log_sink(&self, console: Option<OutputHandle>) -> CapturedLogSink {
        log_record_sink(self.hub.clone(), console)
    }

    pub fn submit_frontend(
        &self,
        entries: Vec<FrontendLogEntryDto>,
    ) -> Result<(), SubmitFrontendLogsError> {
        let entries = validate_frontend_batch(entries)?;
        let pending = entries.into_iter().map(frontend_pending).collect();
        self.hub.publish(pending)?;
        Ok(())
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

#[derive(Debug, thiserror::Error)]
pub enum SubmitFrontendLogsError {
    #[error(transparent)]
    Validation(#[from] FrontendLogValidationError),
    #[error(transparent)]
    Unavailable(#[from] LogsUnavailable),
}

impl SubmitFrontendLogsError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "invalid_frontend_logs",
            Self::Unavailable(_) => "logs_unavailable",
        }
    }
}

fn frontend_pending(entry: ValidatedFrontendLog) -> PendingLog {
    PendingLog {
        timestamp: super::local_timestamp_now(),
        level: entry.level,
        origin: LogOrigin::Frontend,
        domain: entry.domain,
        target: entry.target,
        event: entry.event,
        message: entry.message,
        source: entry.source,
        fields: entry.fields,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use serde_json::json;

    use super::LogRuntime;
    use crate::stream::dispatcher::LogHub;
    use crate::{FrontendLogEntryDto, LogDomain, LogLevel, LogOrigin};

    #[test]
    fn frontend_submission_assigns_stream_metadata_and_frontend_origin() {
        let (hub, dispatcher_guard) = LogHub::start();
        let runtime = LogRuntime {
            hub,
            _dispatcher_guard: Arc::new(dispatcher_guard),
        };
        runtime
            .submit_frontend(vec![FrontendLogEntryDto {
                level: LogLevel::Info,
                domain: LogDomain::Ui,
                target: "editor.canvas".into(),
                event: Some("selectionChanged".into()),
                message: "Selection changed".into(),
                source: Some("main-window".into()),
                fields: BTreeMap::from([("selectedCount".into(), json!(2))]),
            }])
            .unwrap();

        let subscription = runtime.subscribe_batches(|_| true).unwrap();
        assert_eq!(subscription.latest_sequence, 1);
        let record = &subscription.entries[0];
        assert_eq!(record.sequence, 1);
        assert_eq!(record.origin, LogOrigin::Frontend);
        assert_eq!(record.domain, LogDomain::Ui);
        assert_eq!(record.fields["selectedCount"], 2);
        assert!(
            chrono::NaiveDateTime::parse_from_str(&record.timestamp, "%Y-%m-%dT%H:%M:%S%.f")
                .is_ok()
        );
        runtime.unsubscribe(subscription.subscription_id).unwrap();

        runtime
            .submit_frontend(vec![FrontendLogEntryDto {
                level: LogLevel::Debug,
                domain: LogDomain::Ui,
                target: String::new(),
                event: None,
                message: "x".repeat(crate::collector::LogLimits::MAX_MESSAGE_BYTES + 1),
                source: None,
                fields: BTreeMap::new(),
            }])
            .unwrap();
        assert_eq!(
            runtime.subscribe_batches(|_| true).unwrap().latest_sequence,
            1,
            "disabled frontend records are filtered before validation and dispatch"
        );

        let error = runtime.submit_frontend(Vec::new()).unwrap_err();
        assert_eq!(error.code(), "invalid_frontend_logs");
    }
}
