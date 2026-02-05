use crate::graph::GraphData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventFunction {
    FunctionCreated { id: String, data: GraphData },
    FunctionUpdated { id: String, data: GraphData },
    FunctionDeleted { id: String },
}
