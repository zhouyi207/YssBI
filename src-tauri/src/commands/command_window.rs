//! 窗口几何状态的 Tauri 命令。薄包装：直接转发到 [`crate::window_state`]。

use std::collections::HashMap;
use tauri::State;

use crate::error::CommandError;
use crate::window_state::{WindowKind, WindowState, WindowStateStore};

/// 返回所有窗口种类的几何状态（含未保存过时的默认值）。
#[tauri::command]
pub fn get_window_states(store: State<WindowStateStore>) -> HashMap<String, WindowState> {
    let mut out = HashMap::new();
    for kind in WindowKind::all() {
        out.insert(kind.as_str().to_string(), store.get(kind));
    }
    out
}

/// 读取单个窗口种类的几何状态。
#[tauri::command]
pub fn get_window_state(store: State<WindowStateStore>, kind: WindowKind) -> WindowState {
    store.get(kind)
}

/// 写入单个窗口种类的几何状态并立即落盘。
#[tauri::command]
pub fn save_window_state(
    store: State<WindowStateStore>,
    kind: WindowKind,
    value: WindowState,
) -> Result<(), CommandError> {
    store.set(kind, value).map_err(CommandError::internal)
}
