use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub color_theme: String,
    pub activity_bar_position: String,
    pub smooth_scroll: bool,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            color_theme: "Dark Modern (Default)".to_string(),
            activity_bar_position: "Left".to_string(),
            smooth_scroll: true,
        }
    }
}
