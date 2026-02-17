use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    #[serde(rename = "exportTime")]
    pub export_time: String,
    #[serde(rename = "appVersion")]
    pub app_version: String,
}

impl Default for ProjectMetadata {
    fn default() -> Self {
        Self {
            export_time: chrono::Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}
