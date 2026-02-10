//! 项目事件定义和发送逻辑
//!
pub mod event_connection;
pub mod event_dataframe;
pub mod event_event;
pub mod event_function;
pub mod event_macro;
pub mod event_node;
pub mod event_project;
pub mod event_variable;

pub use event_connection::*;
pub use event_dataframe::*;
pub use event_event::*;
pub use event_function::*;
pub use event_macro::*;
pub use event_node::*;
pub use event_project::*;
pub use event_variable::*;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// 项目事件（用于通知前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Event {
    // 项目级事件
    Project(EventProject),
    // Event 子图事件
    Event(EventEvent),
    // Function 子图事件
    Function(EventFunction),
    // Macro 子图事件
    Macro(EventMacro),
    // 全局变量事件
    Variable(EventVariable),
    // 节点事件
    Node(EventNode),
    // 连接事件
    Connection(EventConnection),
    // DataFrame 事件
    DataFrame(EventDataframe),
}

/// 发送项目事件到前端
pub fn emit_project_event(app_handle: &AppHandle, event: Event) {
    use tauri_plugin_log::log::error;
    if let Err(e) = app_handle.emit("project-event", &event) {
        error!("Failed to emit project event: {}", e);
    }
}
