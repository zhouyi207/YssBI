//! 变量 Schema 模块

use crate::graph::value::{DataType, DataValue};
use crate::tabular::display_data_value;
use crate::variable::{VariableInstance, VariableScope};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableInstanceDTO {
    pub id: String,
    pub name: String,
    pub data_type: DataType,
    pub data_value: DataValue,
    pub description: String,
    pub scope: VariableScope,
    pub tags: Vec<String>,
}

impl From<&VariableInstance> for VariableInstanceDTO {
    fn from(value: &VariableInstance) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name.clone(),
            data_type: value.data_type.clone(),
            data_value: display_data_value(value),
            description: value.description.clone(),
            scope: value.scope.clone(),
            tags: value.tags.clone(),
        }
    }
}
