use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

use super::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: ThemeSettings,
    pub editor: EditorSettings,
    pub appearance: AppearanceSettings,
    pub project: ProjectSettings,
    pub window: WindowSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: ThemeSettings::default(),
            editor: EditorSettings::default(),
            appearance: AppearanceSettings::default(),
            project: ProjectSettings::default(),
            window: WindowSettings::default(),
        }
    }
}

fn get_settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut path = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    path.push("settings.json");
    Ok(path)
}

pub fn load_settings(app: &AppHandle) -> AppSettings {
    let path = match get_settings_path(&app) {
        Ok(p) => p,
        Err(_) => return AppSettings::default(),
    };

    if !path.exists() {
        return AppSettings::default();
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return AppSettings::default(),
    };

    serde_json::from_str(&content).unwrap_or_else(|_| AppSettings::default())
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_path(&app)?;
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}
