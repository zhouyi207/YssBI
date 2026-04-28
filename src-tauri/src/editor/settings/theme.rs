//! 设置相关命令

use serde::{Deserialize, Serialize};

// ==================== 数据结构 ====================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct ThemeSettings {
    #[serde(default = "default_theme_mode")]
    pub mode: String,
    pub workbench_background: String,
    pub sidebar_background: String,
    pub accent_color: String,
    pub grid_lines: String,
    pub node_base: String,
    pub connection_lines: String,
    pub selection_region: String,
    pub exec_color: String,
    pub int32_color: String,
    pub int64_color: String,
    pub float32_color: String,
    pub float64_color: String,
    pub int_color: String,
    pub float_color: String,
    pub bool_color: String,
    pub string_color: String,
    pub date_color: String,
    pub datetime_color: String,
    pub categorical_color: String,
    pub dataframe_color: String,
    pub dataseries_color: String,
    pub object_color: String,
    pub any_color: String,
    pub oneof_color: String,
    pub array_color: String,
    pub struct_color: String,
}

fn default_theme_mode() -> String {
    "dark".to_string()
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            mode: default_theme_mode(),
            workbench_background: "#121212".to_string(),
            sidebar_background: "#181818".to_string(),
            accent_color: "#0078d4".to_string(),
            grid_lines: "#252525".to_string(),
            node_base: "#2d2d2d".to_string(),
            connection_lines: "#6b6b6b".to_string(),
            selection_region: "#0078d4".to_string(),
            exec_color: "#ffffff".to_string(),
            int32_color: "#35b2b2".to_string(),
            int64_color: "#2d9d9d".to_string(),
            float32_color: "#9ecd4d".to_string(),
            float64_color: "#8ebd45".to_string(),
            int_color: "#35b2b2".to_string(),
            float_color: "#9ecd4d".to_string(),
            bool_color: "#e06c75".to_string(),
            string_color: "#e5c07b".to_string(),
            date_color: "#c678dd".to_string(),
            datetime_color: "#c678dd".to_string(),
            categorical_color: "#4ec9b0".to_string(),
            dataframe_color: "#61afef".to_string(),
            dataseries_color: "#56b6c2".to_string(),
            object_color: "#abb2bf".to_string(),
            any_color: "#858585".to_string(),
            oneof_color: "#7aabc4".to_string(),
            array_color: "#d19a66".to_string(),
            struct_color: "#b07cd8".to_string(),
        }
    }
}
