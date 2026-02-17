use crate::schema::VariableInstanceDTO;
use crate::variable::{VariableId, VariableScope};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventVariable {
    #[serde(rename_all = "camelCase")]
    VariableCreated {
        variable_id: VariableId,
        variable_scope: VariableScope,
        data: VariableInstanceDTO,
    },
    #[serde(rename_all = "camelCase")]
    VariableUpdated {
        variable_id: VariableId,
        variable_scope: VariableScope,
        data: VariableInstanceDTO,
    },
    #[serde(rename_all = "camelCase")]
    VariableDeleted {
        variable_id: VariableId,
        variable_scope: VariableScope,
    },
}
