use crate::graph::{GraphId};
use serde::{Deserialize, Serialize};
use crate::schema::GraphDTO;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventEvent {
    EventCreated { id: GraphId, data: GraphDTO },
    EventUpdated { id: GraphId, data: GraphDTO },
    EventDeleted { id: GraphId },
}
