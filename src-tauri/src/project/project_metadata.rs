use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub export_time: String,
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
