use crate::graph::GraphData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventMacros {
    MacroCreated { id: String, data: GraphData },
    MacroUpdated { id: String, data: GraphData },
    MacroDeleted { id: String },
}
