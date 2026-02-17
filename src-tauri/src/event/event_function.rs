use crate::graph::GraphId;
use crate::schema::GraphInstanceDTO;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventFunction {
    #[serde(rename_all = "camelCase")]
    FunctionCreated { id: GraphId, data: GraphInstanceDTO },
    #[serde(rename_all = "camelCase")]
    FunctionUpdated { id: GraphId, data: GraphInstanceDTO },
    #[serde(rename_all = "camelCase")]
    FunctionDeleted { id: GraphId },
    #[serde(rename_all = "camelCase")]
    FunctionCreatedFailed { name: String, error: String },
}
