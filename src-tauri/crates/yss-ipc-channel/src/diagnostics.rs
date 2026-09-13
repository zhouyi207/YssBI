//! Connect the neutral diagnostics batch sink to a Tauri channel.
use tauri::ipc::Channel;
use yss_diagnostics::{
    DiagnosticBatchDto, DiagnosticSubscriptionDto, DiagnosticsRuntime, DiagnosticsUnavailable,
};

pub fn subscribe_diagnostics(
    diagnostics: &DiagnosticsRuntime,
    on_records: Channel<DiagnosticBatchDto>,
) -> Result<DiagnosticSubscriptionDto, DiagnosticsUnavailable> {
    diagnostics.subscribe_batches(move |batch| on_records.send(batch).is_ok())
}
