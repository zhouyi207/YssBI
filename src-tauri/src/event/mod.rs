//! 项目事件定义和发送逻辑
//!

pub mod event_project;
pub mod event_resource;

pub use event_project::*;
pub use event_resource::*;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// 项目事件（用于通知前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Event {
    // 项目级事件
    Project(EventProject),

    // 统一资源事件
    Resource(EventResource),
}

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
