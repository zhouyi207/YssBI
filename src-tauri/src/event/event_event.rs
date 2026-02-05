use crate::graph::GraphData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventEvent {
        EventCreated {
        id: String,
        data: GraphData,
    },
    EventUpdated {
        id: String,
        data: GraphData,
    },
    EventDeleted {
        id: String,
    },

}