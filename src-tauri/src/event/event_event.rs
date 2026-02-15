use crate::graph::{GraphId};
use serde::{Deserialize, Serialize};
use crate::schema::GraphInstanceDTO;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventEvent {
    EventCreated { id: GraphId, data: GraphInstanceDTO },
    EventUpdated { id: GraphId, data: GraphInstanceDTO },
    EventDeleted { id: GraphId },
    EventCreatedFailed { name: String, error: String },
}
