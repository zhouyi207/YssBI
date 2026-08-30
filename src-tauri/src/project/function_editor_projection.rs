use crate::graph_document::FunctionParameterId;
use serde::{Deserialize, Serialize};
use yss_data_contract::{DataType, DataTypeParseError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionEditorPin {
    pub id: Box<str>,
    pub name: Box<str>,
    pub data_type: DataType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionEditorProjection {
    pub function_revision: u64,
    pub inputs: Box<[FunctionEditorPin]>,
    pub outputs: Box<[FunctionEditorPin]>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FunctionEditorProjectionError {
    #[error("function editor projection type is invalid")]
    InvalidType(#[source] DataTypeParseError),
    #[error("function editor projection Struct type key is empty")]
    EmptyStructType,
}

pub fn build_function_editor_projection(
    function_revision: u64,
    parameters: impl IntoIterator<Item = (FunctionParameterId, String, String)>,
    return_type: Option<String>,
) -> Result<FunctionEditorProjection, FunctionEditorProjectionError> {
    let inputs = parameters
        .into_iter()
        .map(|(id, name, type_name)| {
            Ok(FunctionEditorPin {
                id: id.as_str().into(),
                name: name.into_boxed_str(),
                data_type: resolve_function_data_type(&type_name)?,
            })
        })
        .collect::<Result<Vec<_>, FunctionEditorProjectionError>>()?
        .into_boxed_slice();
    let outputs = return_type
        .as_deref()
        .map(|return_type| {
            Ok(FunctionEditorPin {
                id: "return".into(),
                name: return_type.into(),
                data_type: resolve_function_data_type(return_type)?,
            })
        })
        .into_iter()
        .collect::<Result<Vec<_>, FunctionEditorProjectionError>>()?
        .into_boxed_slice();
    Ok(FunctionEditorProjection {
        function_revision,
        inputs,
        outputs,
    })
}

pub(crate) fn resolve_function_data_type(
    type_name: &str,
) -> Result<DataType, FunctionEditorProjectionError> {
    let data_type = type_name
        .parse()
        .map_err(|error: DataTypeParseError| FunctionEditorProjectionError::InvalidType(error))?;
    validate_function_data_type(&data_type)?;
    Ok(data_type)
}

fn validate_function_data_type(data_type: &DataType) -> Result<(), FunctionEditorProjectionError> {
    match data_type {
        DataType::Struct(key) if key.trim().is_empty() => {
            Err(FunctionEditorProjectionError::EmptyStructType)
        }
        DataType::Array(inner) | DataType::DataSeries(inner) => validate_function_data_type(inner),
        DataType::OneOf(inner) => inner.iter().try_for_each(validate_function_data_type),
        _ => Ok(()),
    }
}
