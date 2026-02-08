use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettings {
    pub project_name: String,
    pub export_path: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            project_name: "YssBI Project".to_string(),
            export_path: "".to_string(),
        }
    }
}
