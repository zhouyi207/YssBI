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

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

/// 受持久化管理的窗口种类。serde 上以 camelCase 形式与前端 `WindowKind` 对齐。
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum WindowKind {
    Main,
    DatabaseEditor,
    SourceInspector,
    Logs,
    Plot,
    Info,
    Bayes,
}

impl WindowKind {
    pub const ALL: &'static [WindowKind] = &[
        WindowKind::Main,
        WindowKind::DatabaseEditor,
        WindowKind::SourceInspector,
        WindowKind::Logs,
        WindowKind::Plot,
        WindowKind::Info,
        WindowKind::Bayes,
    ];

    /// 用于 HashMap key 的小驼峰字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            WindowKind::Main => "main",
            WindowKind::DatabaseEditor => "databaseEditor",
            WindowKind::SourceInspector => "sourceInspector",
            WindowKind::Logs => "logs",
            WindowKind::Plot => "plot",
            WindowKind::Info => "info",
            WindowKind::Bayes => "bayes",
        }
    }
}

/// 单窗口的几何状态。`x/y` 为物理像素坐标，`None` 表示尚未保存过位置。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub is_maximized: bool,
}

impl WindowState {
    fn default_for(kind: WindowKind) -> Self {
        match kind {
            WindowKind::Main => WindowState {
                width: 1600,
                height: 900,
                x: None,
                y: None,
                is_maximized: false,
            },
            WindowKind::DatabaseEditor | WindowKind::Logs | WindowKind::SourceInspector => {
                WindowState {
                    width: 1000,
                    height: 600,
                    x: None,
                    y: None,
                    is_maximized: false,
                }
            }
            WindowKind::Plot | WindowKind::Info | WindowKind::Bayes => WindowState {
                width: 960,
                height: 800,
                x: None,
                y: None,
                is_maximized: false,
            },
        }
    }
}

/// 文件中持久化的整体结构，缺省值用 `Option` 表示「尚未保存过」。
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedWindowStates {
    #[serde(default)]
    main: Option<WindowState>,
    #[serde(default)]
    database_editor: Option<WindowState>,
    #[serde(default)]
    source_inspector: Option<WindowState>,
    #[serde(default)]
    logs: Option<WindowState>,
    #[serde(default)]
    plot: Option<WindowState>,
    #[serde(default)]
    info: Option<WindowState>,
    #[serde(default)]
    bayes: Option<WindowState>,
}

impl PersistedWindowStates {
    fn get(&self, kind: WindowKind) -> Option<&WindowState> {
        match kind {
            WindowKind::Main => self.main.as_ref(),
            WindowKind::DatabaseEditor => self.database_editor.as_ref(),
            WindowKind::SourceInspector => self.source_inspector.as_ref(),
            WindowKind::Logs => self.logs.as_ref(),
            WindowKind::Plot => self.plot.as_ref(),
            WindowKind::Info => self.info.as_ref(),
            WindowKind::Bayes => self.bayes.as_ref(),
        }
    }

    fn set(&mut self, kind: WindowKind, value: WindowState) {
        match kind {
            WindowKind::Main => self.main = Some(value),
            WindowKind::DatabaseEditor => self.database_editor = Some(value),
            WindowKind::SourceInspector => self.source_inspector = Some(value),
            WindowKind::Logs => self.logs = Some(value),
            WindowKind::Plot => self.plot = Some(value),
            WindowKind::Info => self.info = Some(value),
            WindowKind::Bayes => self.bayes = Some(value),
        }
    }
}

/// Tauri 状态：跨命令访问的窗口几何缓存 + 文件路径。
pub struct WindowStateStore {
    file_path: PathBuf,
    states: Mutex<PersistedWindowStates>,
}

impl WindowStateStore {
    /// 从指定文件加载；文件不存在或解析失败时回退到空状态。
    pub fn load(file_path: PathBuf) -> Self {
        let states = if file_path.exists() {
            fs::read_to_string(&file_path)
                .ok()
                .and_then(|s| serde_json::from_str::<PersistedWindowStates>(&s).ok())
                .unwrap_or_default()
        } else {
            PersistedWindowStates::default()
        };
        Self {
            file_path,
            states: Mutex::new(states),
        }
    }

    /// 读取某 kind 的几何状态，未保存过则返回该 kind 的内置默认值。
    pub fn get(&self, kind: WindowKind) -> WindowState {
        let s = self.states.lock().unwrap();
        s.get(kind)
            .cloned()
            .unwrap_or_else(|| WindowState::default_for(kind))
    }

    /// 写入并立即落盘。
    pub fn set(&self, kind: WindowKind, state: WindowState) -> Result<(), String> {
        {
            let mut s = self.states.lock().unwrap();
            s.set(kind, state);
        }
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        let snapshot = self.states.lock().unwrap().clone();
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
        fs::write(&self.file_path, json).map_err(|e| e.to_string())
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
            .map_err(|e| e.to_string())?;
    }
    win.set_size(PhysicalSize::new(state.width, state.height))
        .map_err(|e| e.to_string())?;
    if state.is_maximized {
        win.maximize().map_err(|e| e.to_string())?;
    }
    win.show().map_err(|e| e.to_string())?;
    Ok(())
}
