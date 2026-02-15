use crate::graph::GraphId;
use crate::schema::GraphInstanceDTO;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventFunction {
    FunctionCreated { id: GraphId, data: GraphInstanceDTO },
    FunctionUpdated { id: GraphId, data: GraphInstanceDTO },
    FunctionDeleted { id: GraphId },
    FunctionCreatedFailed { name: String, error: String },
}
