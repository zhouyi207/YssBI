use crate::project::ProjectData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventProject {
    #[serde(rename_all = "camelCase")]
    ProjectLoaded {
        data: ProjectData,
        path: Option<String>,
    },
    ProjectCleared,
    #[serde(rename_all = "camelCase")]
    ProjectSaved {
        path: String,
    },
}
