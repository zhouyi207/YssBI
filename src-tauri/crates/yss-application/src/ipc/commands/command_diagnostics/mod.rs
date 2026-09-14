use tauri::State;
use tauri::ipc::Channel;

use crate::ipc::error::CommandError;
use yss_diagnostics::{
    DiagnosticBatchDto, DiagnosticSubmissionError, DiagnosticSubscriptionDto, DiagnosticsRuntime,
    FrontendDiagnosticEntryDto,
};

#[tauri::command]
pub fn submit_frontend_diagnostics(
    diagnostics: State<'_, DiagnosticsRuntime>,
    entries: Vec<FrontendDiagnosticEntryDto>,
) -> Result<(), CommandError> {
    diagnostics
        .submit_frontend(entries)
        .map_err(|error| match error {
            DiagnosticSubmissionError::Validation(_) => {
                CommandError::expected("invalid_frontend_diagnostics")
            }
            DiagnosticSubmissionError::Unavailable(error) => {
                CommandError::diagnosed("diagnostics_unavailable", error)
            }
        })
}

#[tauri::command]
pub fn subscribe_diagnostics(
    diagnostics: State<'_, DiagnosticsRuntime>,
    on_records: Channel<DiagnosticBatchDto>,
) -> Result<DiagnosticSubscriptionDto, CommandError> {
    yss_ipc_channel::diagnostics::subscribe_diagnostics(&diagnostics, on_records)
        .map_err(|error| CommandError::diagnosed("diagnostics_unavailable", error))
}

#[tauri::command]
pub fn unsubscribe_diagnostics(
    diagnostics: State<'_, DiagnosticsRuntime>,
    subscription_id: String,
) -> Result<(), CommandError> {
    diagnostics
        .unsubscribe(subscription_id)
        .map_err(|error| CommandError::diagnosed("diagnostics_unavailable", error))
}
