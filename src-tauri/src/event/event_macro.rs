use crate::graph::{GraphId};
use serde::{Deserialize, Serialize};
use crate::schema::GraphInstanceDTO;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventMacro {
    #[serde(rename_all = "camelCase")]
    MacroCreated { id: GraphId, data: GraphInstanceDTO },
    #[serde(rename_all = "camelCase")]
    MacroUpdated { id: GraphId, data: GraphInstanceDTO },
    #[serde(rename_all = "camelCase")]
    MacroDeleted { id: GraphId },
    #[serde(rename_all = "camelCase")]
    MacroCreatedFailed { name: String, error: String },
}
