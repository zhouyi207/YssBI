//! 项目事件定义和发送逻辑
//!

pub mod event_dataframe;

pub mod event_project;
pub mod event_resource;
pub mod event_variable;

pub use event_dataframe::*;

pub use event_project::*;
pub use event_resource::*;
pub use event_variable::*;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// 项目事件（用于通知前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Event {
    // 项目级事件
    Project(EventProject),

    // 全局变量事件
    Variable(EventVariable),

    // DataFrame 事件
    DataFrame(EventDataframe),
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
    use tauri_plugin_log::log::error;
    if let Err(error) = emit_project_event_result(app_handle, &event) {
        error!("Failed to emit project event: {error}");
    }
}

/// Notify the frontend to refresh the on-disk project index snapshot.
pub fn emit_project_index_invalidated(app_handle: &AppHandle, source: impl Into<String>) {
    let version = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    emit_project_event(
        app_handle,
        Event::Resource(EventResource::ProjectIndexInvalidated {
            source: source.into(),
            version,
        }),
    );
}
