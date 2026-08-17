use tauri::State;
use tauri::ipc::Channel;

use crate::diagnostics::{
    DiagnosticBatchDto, DiagnosticSubscriptionDto, DiagnosticsRuntime, FrontendDiagnosticEntryDto,
};
use crate::error::CommandError;

#[tauri::command]
pub fn submit_frontend_diagnostics(
    diagnostics: State<'_, DiagnosticsRuntime>,
    entries: Vec<FrontendDiagnosticEntryDto>,
) -> Result<(), CommandError> {
    diagnostics.submit_frontend(entries).map_err(|error| {
        if error.code() == "invalid_frontend_diagnostics" {
            CommandError::expected("invalid_frontend_diagnostics")
        } else {
            CommandError::diagnosed("diagnostics_unavailable", error)
        }
    })
}

#[tauri::command]
pub fn subscribe_diagnostics(
    diagnostics: State<'_, DiagnosticsRuntime>,
    on_records: Channel<DiagnosticBatchDto>,
) -> Result<DiagnosticSubscriptionDto, CommandError> {
    diagnostics
        .subscribe(on_records)
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
