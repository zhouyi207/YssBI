use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventDataframe {
        DataFrameCreated {
        id: String,
        // data: DataFrameData,
    },
    DataFrameDeleted {
        id: String,
    },
}