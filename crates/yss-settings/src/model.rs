use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;
use yss_harness_contract::{HarnessMode, ReasoningEffort};

pub const UI_FONT_SIZES: RangeInclusive<f32> = 8.0..=32.0;
pub const MONO_FONT_SIZES: RangeInclusive<f32> = 8.0..=48.0;
pub const AUTOSAVE_DELAYS_MS: RangeInclusive<u64> = 250..=60_000;
pub const TABLE_PAGE_SIZES: RangeInclusive<usize> = 1..=10_000;
pub const CHART_PREVIEW_POINTS: RangeInclusive<usize> = 1..=100_000;
pub const LOG_RETENTION_DAYS: RangeInclusive<u32> = 1..=3650;
pub const LOG_STORAGE_MIB: RangeInclusive<u32> = 1..=10240;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct UserSettings {
    pub language: Language,
    pub appearance: AppearanceSettings,
    pub editor: EditorSettings,
    pub tables: TableSettings,
    pub workspace: WorkspaceSettings,
    pub assistant: AssistantSettings,
    pub logs: LogSettings,
    pub charts: ChartSettings,
    pub keybindings: Vec<KeyBindingOverride>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "zh-CN")]
    Chinese,
    #[serde(rename = "en-US")]
    English,
}
impl Language {
    pub fn locale(self) -> &'static str {
        match self {
            Self::Chinese => yss_i18n::CHINESE,
            Self::English => yss_i18n::ENGLISH,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    System,
    Light,
    #[default]
    Dark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct AppearanceSettings {
    pub mode: ThemeMode,
    pub ui_font_family: Option<String>,
    pub ui_font_size: f32,
    pub mono_font_family: Option<String>,
    pub mono_font_size: f32,
}
impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Dark,
            ui_font_family: None,
            ui_font_size: 14.,
            mono_font_family: None,
            mono_font_size: 12.,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Autosave {
    #[default]
    Off,
    AfterDelay,
    OnFocusChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct EditorSettings {
    pub soft_wrap: bool,
    pub line_numbers: bool,
    pub open_in_preview: bool,
    pub autosave: Autosave,
    pub autosave_delay_ms: u64,
}
impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            soft_wrap: true,
            line_numbers: true,
            open_in_preview: false,
            autosave: Autosave::Off,
            autosave_delay_ms: 1000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    #[default]
    Compact,
    Standard,
    Comfortable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct TableSettings {
    pub density: Density,
    pub stripe: bool,
    pub page_size: usize,
}
impl Default for TableSettings {
    fn default() -> Self {
        Self {
            density: Density::Compact,
            stripe: true,
            page_size: 100,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct WorkspaceSettings {
    pub restore_layout: bool,
    pub show_details: bool,
    pub show_bottom_panel: bool,
    pub restore_last_project: bool,
}
impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            restore_layout: true,
            show_details: true,
            show_bottom_panel: false,
            restore_last_project: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct AssistantSettings {
    pub mode: HarnessMode,
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct LogSettings {
    pub auto_scroll: bool,
    pub visible_levels: Vec<LogLevel>,
    pub retention_days: Option<u32>,
    pub max_storage_mib: Option<u32>,
}
impl Default for LogSettings {
    fn default() -> Self {
        Self {
            auto_scroll: true,
            visible_levels: vec![
                LogLevel::Error,
                LogLevel::Warn,
                LogLevel::Info,
                LogLevel::Debug,
                LogLevel::Trace,
            ],
            retention_days: None,
            max_storage_mib: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct ChartSettings {
    pub max_preview_points: usize,
}
impl Default for ChartSettings {
    fn default() -> Self {
        Self {
            max_preview_points: 10_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct KeyBindingOverride {
    pub action: String,
    pub context: Option<String>,
    pub keystroke: String,
}
impl KeyBindingOverride {
    pub fn new(action: String, context: Option<String>, keystroke: String) -> Self {
        Self {
            action,
            context,
            keystroke,
        }
    }
}

impl UserSettings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            UI_FONT_SIZES.contains(&self.appearance.ui_font_size),
            "UI font size must be between {} and {}",
            UI_FONT_SIZES.start(),
            UI_FONT_SIZES.end()
        );
        ensure!(
            MONO_FONT_SIZES.contains(&self.appearance.mono_font_size),
            "Document font size must be between {} and {}",
            MONO_FONT_SIZES.start(),
            MONO_FONT_SIZES.end()
        );
        for family in [
            &self.appearance.ui_font_family,
            &self.appearance.mono_font_family,
        ]
        .into_iter()
        .flatten()
        {
            ensure!(
                !family.trim().is_empty()
                    && family.len() <= 256
                    && !family.chars().any(char::is_control),
                "Invalid font family"
            );
        }
        ensure!(
            AUTOSAVE_DELAYS_MS.contains(&self.editor.autosave_delay_ms),
            "Autosave delay must be between {} and {} milliseconds",
            AUTOSAVE_DELAYS_MS.start(),
            AUTOSAVE_DELAYS_MS.end()
        );
        ensure!(
            TABLE_PAGE_SIZES.contains(&self.tables.page_size),
            "Page size must be between {} and {}",
            TABLE_PAGE_SIZES.start(),
            TABLE_PAGE_SIZES.end()
        );
        ensure!(
            CHART_PREVIEW_POINTS.contains(&self.charts.max_preview_points),
            "Chart preview limit must be between {} and {}",
            CHART_PREVIEW_POINTS.start(),
            CHART_PREVIEW_POINTS.end()
        );
        if let Some(days) = self.logs.retention_days {
            ensure!(
                LOG_RETENTION_DAYS.contains(&days),
                "Log retention must be between {} and {} days",
                LOG_RETENTION_DAYS.start(),
                LOG_RETENTION_DAYS.end()
            );
        }
        if let Some(size) = self.logs.max_storage_mib {
            ensure!(
                LOG_STORAGE_MIB.contains(&size),
                "Log storage target must be between {} and {} MiB",
                LOG_STORAGE_MIB.start(),
                LOG_STORAGE_MIB.end()
            );
        }
        ensure!(
            self.logs
                .visible_levels
                .iter()
                .enumerate()
                .all(|(index, level)| !self
                    .logs
                    .visible_levels
                    .iter()
                    .take(index)
                    .any(|previous| previous == level)),
            "Duplicate log levels"
        );
        for (index, binding) in self.keybindings.iter().enumerate() {
            ensure!(
                !binding.action.trim().is_empty(),
                "Key binding requires an action"
            );
            ensure!(
                !self
                    .keybindings
                    .iter()
                    .take(index)
                    .any(|previous| previous.context == binding.context
                        && previous.action == binding.action),
                "Duplicate key binding override"
            );
            ensure!(
                binding.keystroke.is_empty()
                    || !self
                        .keybindings
                        .iter()
                        .take(index)
                        .any(|previous| previous.context == binding.context
                            && previous.keystroke == binding.keystroke),
                "Conflicting key bindings in the same context"
            );
        }
        Ok(())
    }
}
