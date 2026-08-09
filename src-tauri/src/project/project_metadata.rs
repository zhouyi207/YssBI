use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    #[serde(default)]
    pub project_name: String,
    pub export_time: String,
}

impl Default for ProjectMetadata {
    fn default() -> Self {
        Self {
            project_name: "未命名项目".to_string(),
            export_time: chrono::Utc::now().to_rfc3339(),
        }
    }
}
