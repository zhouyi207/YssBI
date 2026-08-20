//! 多窗口几何状态（位置/尺寸/最大化）持久化。
//!
//! 后端是 window state 的权威来源：
//! - 启动时 `tauri::Builder::setup` 调用 [`apply_main_window_state`] 在 `show()`
//!   前把保存的尺寸/位置应用到主窗口，避免「先以默认尺寸显示，再被前端缩放」
//!   的闪烁。
//! - 前端通过 [`crate::commands::get_window_state`] /
//!   [`crate::commands::save_window_state`] 读写各 kind 的状态，子窗口在创建
//!   时直接以保存的尺寸/位置启动。
//!
//! 文件位置：`<app_config_dir>/window_state.json`。

mod kind;
mod persistence;
#[cfg(test)]
mod tests;

pub use kind::{WindowKind, WindowState};

use kind::PersistedWindowStates;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

/// Tauri 状态：跨命令访问的窗口几何缓存 + 文件路径。
pub struct WindowStateStore {
    file_path: PathBuf,
    states: Mutex<PersistedWindowStates>,
    set_lock: Mutex<()>,
}

impl WindowStateStore {
    /// 从指定文件加载；文件不存在或解析失败时回退到空状态。
    pub fn load(file_path: PathBuf) -> Self {
        let states = if file_path.exists() {
            fs::read_to_string(&file_path)
                .ok()
                .and_then(|content| serde_json::from_str::<PersistedWindowStates>(&content).ok())
                .unwrap_or_default()
        } else {
            PersistedWindowStates::default()
        };
        Self {
            file_path,
            states: Mutex::new(states),
            set_lock: Mutex::new(()),
        }
    }

    /// 读取某 kind 的几何状态，未保存过则返回该 kind 的内置默认值。
    pub fn get(&self, kind: WindowKind) -> WindowState {
        let states = self.states.lock().unwrap();
        states
            .get(kind)
            .cloned()
            .unwrap_or_else(|| WindowState::default_for(kind))
    }

    /// 写入并立即落盘。只有 candidate snapshot 持久化成功后才提交内存状态。
    pub fn set(&self, kind: WindowKind, state: WindowState) -> Result<(), String> {
        let _set_guard = self.set_lock.lock().unwrap();
        let mut candidate = self.states.lock().unwrap().clone();
        candidate.set(kind, state);

        self.persist(&candidate)?;
        *self.states.lock().unwrap() = candidate;
        Ok(())
    }

    fn persist(&self, snapshot: &PersistedWindowStates) -> Result<(), String> {
        persistence::write_json_atomically(&self.file_path, snapshot)
    }
}

/// 在 setup 阶段把主窗口几何状态应用上去并 `show()`，避免视觉闪烁。
///
/// 主窗口在 `tauri.conf.json` 中应当配置为 `visible: false`，由本函数负责展示。
pub fn apply_main_window_state(app: &AppHandle, store: &WindowStateStore) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let state = store.get(WindowKind::Main);

    if let (Some(x), Some(y)) = (state.x, state.y) {
        win.set_position(PhysicalPosition::new(x, y))
            .map_err(|error| error.to_string())?;
    }
    win.set_size(PhysicalSize::new(state.width, state.height))
        .map_err(|error| error.to_string())?;
    if state.is_maximized {
        win.maximize().map_err(|error| error.to_string())?;
    }
    win.show().map_err(|error| error.to_string())?;
    Ok(())
}
