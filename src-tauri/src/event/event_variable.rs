use crate::variable::VariableDefinition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventVariable {
    #[serde(rename_all = "camelCase")]
    GlobalVariableCreated {
        id: String,
        data: VariableDefinition,
    },
    #[serde(rename_all = "camelCase")]
    GlobalVariableUpdated {
        id: String,
        data: VariableDefinition,
    },
    #[serde(rename_all = "camelCase")]
    GlobalVariableDeleted {
        id: String,
    },
    #[serde(rename_all = "camelCase")]
    LocalVariableCreated {
        subgraph_id: String,
        variable_id: String,
        data: VariableDefinition,
    },
    #[serde(rename_all = "camelCase")]
    LocalVariableUpdated {
        subgraph_id: String,
        variable_id: String,
        data: VariableDefinition,
    },
    #[serde(rename_all = "camelCase")]
    LocalVariableDeleted {
        subgraph_id: String,
        variable_id: String,
    },
}
