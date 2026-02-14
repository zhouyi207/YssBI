use crate::graph::{GraphId};
use serde::{Deserialize, Serialize};
use crate::schema::GraphInstanceDTO;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventMacro {
    MacroCreated { id: GraphId, data: GraphInstanceDTO },
    MacroUpdated { id: GraphId, data: GraphInstanceDTO },
    MacroDeleted { id: GraphId },
}
