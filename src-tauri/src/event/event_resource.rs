use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResourceMetaEvent {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub uri: String,
    pub folder_path: Option<String>,
    pub exists: bool,
    pub loaded: bool,
    pub has_dirty_document: bool,
    pub has_stale_document: bool,
    pub has_conflict_document: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventResource {
    #[serde(rename_all = "camelCase")]
    ResourceChanged {
        id: String,
        kind: String,
        source: String,
        data: ProjectResourceMetaEvent,
    },
    #[serde(rename_all = "camelCase")]
    ResourceDeleted {
        id: String,
        kind: String,
        source: String,
    },
}
