use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettings {
    pub show_grid: bool,
    pub auto_save: bool,
    pub snap_to_grid: bool,
    pub font_size: u32,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            show_grid: true,
            auto_save: true,
            snap_to_grid: true,
            font_size: 12,
        }
    }
}
