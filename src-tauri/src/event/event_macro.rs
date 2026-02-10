use crate::graph::{GraphId};
use serde::{Deserialize, Serialize};
use crate::schema::GraphDTO;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventMacro {
    MacroCreated { id: GraphId, data: GraphDTO },
    MacroUpdated { id: GraphId, data: GraphDTO },
    MacroDeleted { id: GraphId },
}
