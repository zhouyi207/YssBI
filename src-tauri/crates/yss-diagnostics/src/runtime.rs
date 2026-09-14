use super::dispatcher::{
    DiagnosticsDispatcherGuard, DiagnosticsDispatcherStartError, DiagnosticsHub,
    DiagnosticsUnavailable, PendingDiagnostic,
};
use super::dto::{
    DiagnosticBatchDto, DiagnosticEvent, DiagnosticOrigin, DiagnosticSubscriptionDto,
    FrontendDiagnosticEntryDto,
};
use super::validation::{
    DiagnosticValidationError, ValidatedDiagnostic, validate_entry, validate_frontend_batch,
};

/// Owns the diagnostic recent ring and live subscriber dispatcher.
///
/// Accepts explicit diagnostic data without installing or reading any logging runtime.
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

    /// Publish diagnostic data produced by a backend use case.
    pub fn publish(&self, event: DiagnosticEvent) -> Result<(), DiagnosticSubmissionError> {
        let entry = validate_entry(0, event)?;
        self.hub
            .publish(vec![pending(&entry, DiagnosticOrigin::Rust)])?;
        Ok(())
    }

    pub fn submit_frontend(
        &self,
        entries: Vec<FrontendDiagnosticEntryDto>,
    ) -> Result<(), DiagnosticSubmissionError> {
        let entries = validate_frontend_batch(entries)?;
        let pending = entries
            .iter()
            .map(|entry| pending(entry, DiagnosticOrigin::Frontend))
            .collect();
        self.hub.publish(pending)?;
        Ok(())
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
pub enum DiagnosticSubmissionError {
    #[error(transparent)]
    Validation(#[from] DiagnosticValidationError),
    #[error(transparent)]
    Unavailable(#[from] DiagnosticsUnavailable),
}

fn pending(entry: &ValidatedDiagnostic, origin: DiagnosticOrigin) -> PendingDiagnostic {
    PendingDiagnostic {
        timestamp: super::local_timestamp_now(),
        level: entry.level,
        origin,
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
    use std::sync::mpsc;
    use std::time::Duration;

    use serde_json::json;

    use super::DiagnosticsRuntime;
    use crate::dispatcher::DiagnosticsHub;
    use crate::{
        DiagnosticDomain, DiagnosticEvent, DiagnosticLevel, DiagnosticOrigin,
        FrontendDiagnosticEntryDto,
    };

    #[test]
    fn backend_data_reaches_snapshot_and_live_delivery_without_a_logging_runtime() {
        let runtime = DiagnosticsRuntime::initialize().unwrap();
        let event = DiagnosticEvent {
            level: DiagnosticLevel::Warn,
            domain: DiagnosticDomain::Data,
            target: "dataset.validation".into(),
            event: Some("missingValues".into()),
            message: "Dataset contains missing values".into(),
            source: None,
            fields: BTreeMap::from([
                ("missingCount".into(), json!(3)),
                ("password".into(), json!("secret")),
            ]),
        };
        runtime.publish(event.clone()).unwrap();
        let (sender, receiver) = mpsc::channel();
        let snapshot = runtime
            .subscribe_batches(move |batch| sender.send(batch).is_ok())
            .unwrap();
        assert_eq!(snapshot.latest_sequence, 1);
        let record = &snapshot.entries[0];
        assert_eq!(record.origin, DiagnosticOrigin::Rust);
        assert_eq!(record.fields["missingCount"], 3);
        assert_eq!(record.fields["password"], "[REDACTED]");

        runtime.publish(event.clone()).unwrap();
        let batch = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(batch.stream_id, snapshot.stream_id);
        assert_eq!(batch.entries[0].sequence, 2);
        assert_eq!(batch.entries[0].event.as_deref(), Some("missingValues"));

        let invalid = DiagnosticEvent {
            target: String::new(),
            ..event
        };
        assert!(matches!(
            runtime.publish(invalid),
            Err(super::DiagnosticSubmissionError::Validation(_))
        ));
        assert_eq!(
            runtime.subscribe_batches(|_| true).unwrap().latest_sequence,
            2
        );
    }

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
        assert!(
            chrono::NaiveDateTime::parse_from_str(&record.timestamp, "%Y-%m-%dT%H:%M:%S%.f")
                .is_ok()
        );
        runtime.unsubscribe(subscription.subscription_id).unwrap();

        let error = runtime.submit_frontend(Vec::new()).unwrap_err();
        assert!(matches!(
            error,
            super::DiagnosticSubmissionError::Validation(_)
        ));
    }
}
