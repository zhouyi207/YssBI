use crate::project::ProjectData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventProject {
    ProjectLoaded {
        data: ProjectData,
        path: Option<String>,
    },
    ProjectCleared,
    ProjectSaved {
        path: String,
    },
}
