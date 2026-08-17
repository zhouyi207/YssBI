use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventDataframe {
    #[serde(rename_all = "camelCase")]
    DataFrameCreated { id: String },
    #[serde(rename_all = "camelCase")]
    DataFrameDeleted { id: String },
}
