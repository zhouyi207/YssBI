//! 设置相关命令

use serde::{Deserialize, Serialize};

// ==================== 数据结构 ====================

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

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            workbench_background: "#121212".to_string(),
            sidebar_background: "#181818".to_string(),
            accent_color: "#0078d4".to_string(),
            grid_lines: "#252525".to_string(),
            node_base: "#2d2d2d".to_string(),
            connection_lines: "#6b6b6b".to_string(),
            selection_region: "#0078d4".to_string(),
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
        }
    }
}
