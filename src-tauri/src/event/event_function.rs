use crate::schema::GraphInstanceDTO;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventFunction {
    #[serde(rename_all = "camelCase")]
    FunctionCreated { path: String, data: GraphInstanceDTO },
    #[serde(rename_all = "camelCase")]
    FunctionUpdated { path: String, data: GraphInstanceDTO },
    #[serde(rename_all = "camelCase")]
    FunctionDeleted { path: String },
    #[serde(rename_all = "camelCase")]
    FunctionCreatedFailed { name: String, error: String },
}
