use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowSettings {
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub is_maximized: bool,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            width: 1600,
            height: 900,
            x: None,
            y: None,
            is_maximized: false,
        }
    }
}
