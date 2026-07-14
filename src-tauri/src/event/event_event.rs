use crate::schema::GraphInstanceDTO;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventEvent {
    #[serde(rename_all = "camelCase")]
    EventUpdated { path: String, data: GraphInstanceDTO },
    #[serde(rename_all = "camelCase")]
    EventDeleted { path: String },
}
