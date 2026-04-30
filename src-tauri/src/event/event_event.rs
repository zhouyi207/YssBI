use crate::graph::GraphId;
use crate::schema::GraphInstanceDTO;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventEvent {
    #[serde(rename_all = "camelCase")]
    EventCreated { id: GraphId, data: GraphInstanceDTO },
    #[serde(rename_all = "camelCase")]
    EventUpdated { id: GraphId, data: GraphInstanceDTO },
    #[serde(rename_all = "camelCase")]
    EventDeleted { id: GraphId },
    #[serde(rename_all = "camelCase")]
    EventCreatedFailed { name: String, error: String },
}
