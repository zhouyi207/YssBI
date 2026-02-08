use crate::editor::settings;
use crate::editor::settings::AppSettings;
use tauri::AppHandle;

#[tauri::command]
pub fn load_settings(app: AppHandle) -> AppSettings {
    settings::load_settings(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    settings::save_settings(&app, &settings)
}
