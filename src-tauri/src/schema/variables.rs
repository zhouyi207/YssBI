//! Variable transport mapping.

use serde::{Deserialize, Serialize};
use yss_data_contract::{DataSeriesValue, DataType, DataValue};
use yss_variable_contract::{VariableInstance, VariableScope};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableDtoMappingErrorCode {
    JsonEncodeFailed,
}

#[derive(Debug, thiserror::Error)]
#[error("variable DTO mapping failed")]
pub struct VariableDtoMappingError {
    code: VariableDtoMappingErrorCode,
    #[source]
    source: serde_json::Error,
}

impl VariableDtoMappingError {
    pub fn code(&self) -> VariableDtoMappingErrorCode {
        self.code
    }
}

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

impl TryFrom<&VariableInstance> for VariableInstanceDTO {
    type Error = VariableDtoMappingError;

    fn try_from(value: &VariableInstance) -> Result<Self, Self::Error> {
        let data_value = if let Some(snapshot) = &value.tabular {
            let literal = serde_json::to_string(&snapshot.columns_view()).map_err(|source| {
                VariableDtoMappingError {
                    code: VariableDtoMappingErrorCode::JsonEncodeFailed,
                    source,
                }
            })?;
            match &value.data_type {
                DataType::DataFrame => DataValue::DataFrame(literal),
                DataType::DataSeries(_) => DataValue::DataSeries(DataSeriesValue::new(literal)),
                _ => value.data_value.clone(),
            }
        } else {
            value.data_value.clone()
        };

        Ok(Self {
            id: value.id.to_string(),
            name: value.name.clone(),
            data_type: value.data_type.clone(),
            data_value,
            description: value.description.clone(),
            scope: value.scope.clone(),
            tags: value.tags.clone(),
        })
    }
}
