//! Tauri event delivery after authoritative business commits.
use tauri::{AppHandle, Emitter};
use yss_ipc_contract::event::Event;

/// 发送项目事件到前端
pub fn emit_project_event_result(app_handle: &AppHandle, event: &Event) -> Result<(), String> {
    app_handle
        .emit("project-event", event)
        .map_err(|error| error.to_string())
}

pub fn emit_project_event(app_handle: &AppHandle, event: Event) {
    if let Err(error) = emit_project_event_result(app_handle, &event) {
        tracing::error!(
            target: "yssbi::project::events",
            diagnostic_domain = "application",
            diagnostic_event = "projectEventEmitFailed",
            error = %error,
            "Failed to emit project event"
        );
    }
}
