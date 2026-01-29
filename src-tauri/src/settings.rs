use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSettings {
    pub workbench_background: String,
    pub sidebar_background: String,
    pub accent_color: String,
    pub grid_lines: String,
    pub node_base: String,
    pub connection_lines: String,
    pub selection_region: String,
    pub exec_color: String,
    pub int_color: String,
    pub float_color: String,
    pub bool_color: String,
    pub string_color: String,
    pub date_color: String,
    pub datetime_color: String,
    pub dataframe_color: String,
    pub object_color: String,
    pub array_color: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettings {
    pub show_grid: bool,
    pub auto_save: bool,
    pub snap_to_grid: bool,
    pub font_size: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub color_theme: String,
    pub activity_bar_position: String,
    pub smooth_scroll: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettings {
    pub project_name: String,
    pub export_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowSettings {
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub is_maximized: bool,
}

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
            theme: ThemeSettings {
                workbench_background: "#121212".to_string(),
                sidebar_background: "#181818".to_string(),
                accent_color: "#0078d4".to_string(),
                grid_lines: "#252525".to_string(),
                node_base: "#2d2d2d".to_string(),
                connection_lines: "#6b6b6b".to_string(),
                selection_region: "#0078d433".to_string(),
                exec_color: "#ffffff".to_string(),
                int_color: "#35b2b2".to_string(),
                float_color: "#9ecd4d".to_string(),
                bool_color: "#e06c75".to_string(),
                string_color: "#e5c07b".to_string(),
                date_color: "#c678dd".to_string(),
                datetime_color: "#c678dd".to_string(),
                dataframe_color: "#61afef".to_string(),
                object_color: "#abb2bf".to_string(),
                array_color: "#d19a66".to_string(),
            },
            editor: EditorSettings {
                show_grid: true,
                auto_save: true,
                snap_to_grid: true,
                font_size: 12,
            },
            appearance: AppearanceSettings {
                color_theme: "Dark Modern (Default)".to_string(),
                activity_bar_position: "Left".to_string(),
                smooth_scroll: true,
            },
            project: ProjectSettings {
                project_name: "YssBI Project".to_string(),
                export_path: "".to_string(),
            },
            window: WindowSettings {
                width: 1600,
                height: 900,
                x: None,
                y: None,
                is_maximized: false,
            },
        }
    }
}

fn get_settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut path = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    path.push("settings.json");
    Ok(path)
}

#[tauri::command]
pub fn load_settings(app: AppHandle) -> AppSettings {
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

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    let path = get_settings_path(&app)?;
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}
