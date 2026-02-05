use crate::variable::VariableDefinition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventVariable {
    GlobalVariableCreated {
        id: String,
        data: VariableDefinition,
    },
    GlobalVariableUpdated {
        id: String,
        data: VariableDefinition,
    },
    GlobalVariableDeleted {
        id: String,
    },

    LocalVariableCreated {
        subgraph_id: String,
        variable_id: String,
        data: VariableDefinition,
    },
    LocalVariableUpdated {
        subgraph_id: String,
        variable_id: String,
        data: VariableDefinition,
    },
    LocalVariableDeleted {
        subgraph_id: String,
        variable_id: String,
    },
}
