use crate::graph::GraphId;
use crate::schema::GraphDTO;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventFunction {
    FunctionCreated { id: GraphId, data: GraphDTO },
    FunctionUpdated { id: GraphId, data: GraphDTO },
    FunctionDeleted { id: GraphId },
}
