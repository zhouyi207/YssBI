use tauri::ipc::Channel;
use yss_tracing::LogRecordSink;

use super::dispatcher::{
    DiagnosticsDispatcherGuard, DiagnosticsDispatcherStartError, DiagnosticsHub,
    DiagnosticsUnavailable, PendingDiagnostic,
};
use super::dto::{
    DiagnosticBatchDto, DiagnosticOrigin, DiagnosticSubscriptionDto, FrontendDiagnosticEntryDto,
};
use super::rust_projection::log_record_sink;
use super::validation::{
    FrontendDiagnosticValidationError, ValidatedFrontendDiagnostic, validate_frontend_batch,
};

/// Owns the diagnostic recent ring and live subscriber dispatcher.
///
/// Logging configuration and persistent outputs are intentionally owned by
/// `yss-tracing`; this runtime only accepts its sanitized record projection.
pub struct DiagnosticsRuntime {
    hub: DiagnosticsHub,
    _dispatcher_guard: DiagnosticsDispatcherGuard,
}

impl DiagnosticsRuntime {
    pub fn initialize() -> Result<Self, DiagnosticsInitializationError> {
        let (hub, dispatcher_guard) = DiagnosticsHub::start_production()?;
        Ok(Self {
            hub,
            _dispatcher_guard: dispatcher_guard,
        })
    }

    pub fn rust_log_sink(&self) -> LogRecordSink {
        log_record_sink(self.hub.clone())
    }

    pub fn submit_frontend(
        &self,
        entries: Vec<FrontendDiagnosticEntryDto>,
    ) -> Result<(), SubmitFrontendDiagnosticsError> {
        let entries = validate_frontend_batch(entries)?;
        let pending = entries.iter().map(frontend_pending).collect();
        self.hub.publish(pending)?;
        Ok(())
    }

    pub fn subscribe(
        &self,
        on_records: Channel<DiagnosticBatchDto>,
    ) -> Result<DiagnosticSubscriptionDto, DiagnosticsUnavailable> {
        self.subscribe_batches(move |batch| on_records.send(batch).is_ok())
    }

    /// Subscribes a platform-neutral bounded batch sink and returns the
    /// current recent snapshot at the same ordered stream boundary.
    pub fn subscribe_batches(
        &self,
        on_records: impl Fn(DiagnosticBatchDto) -> bool + Send + 'static,
    ) -> Result<DiagnosticSubscriptionDto, DiagnosticsUnavailable> {
        self.hub.subscribe(on_records)
    }

    pub fn unsubscribe(&self, subscription_id: String) -> Result<(), DiagnosticsUnavailable> {
        self.hub.unsubscribe(subscription_id)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("failed to initialize diagnostics runtime")]
pub struct DiagnosticsInitializationError {
    #[source]
    source: DiagnosticsDispatcherStartError,
}

impl From<DiagnosticsDispatcherStartError> for DiagnosticsInitializationError {
    fn from(source: DiagnosticsDispatcherStartError) -> Self {
        Self { source }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubmitFrontendDiagnosticsError {
    #[error(transparent)]
    Validation(#[from] FrontendDiagnosticValidationError),
    #[error(transparent)]
    Unavailable(#[from] DiagnosticsUnavailable),
}

impl SubmitFrontendDiagnosticsError {
    pub const fn code(&self) -> &'static str {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::DiagnosticsRuntime;
    use crate::dispatcher::DiagnosticsHub;
    use crate::{DiagnosticDomain, DiagnosticLevel, DiagnosticOrigin, FrontendDiagnosticEntryDto};

    #[test]
    fn frontend_submission_assigns_stream_metadata_and_frontend_origin() {
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

        let subscription = runtime.subscribe_batches(|_| true).unwrap();
        assert_eq!(subscription.latest_sequence, 1);
        let record = &subscription.entries[0];
        assert_eq!(record.sequence, 1);
        assert_eq!(record.origin, DiagnosticOrigin::Frontend);
        assert_eq!(record.domain, DiagnosticDomain::Ui);
        assert_eq!(record.fields["selectedCount"], 2);
        assert!(chrono::DateTime::parse_from_rfc3339(&record.timestamp).is_ok());
        runtime.unsubscribe(subscription.subscription_id).unwrap();

        let error = runtime.submit_frontend(Vec::new()).unwrap_err();
        assert_eq!(error.code(), "invalid_frontend_diagnostics");
    }
}
