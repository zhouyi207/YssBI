use crate::data_contract::{DataType, DataTypeParseError};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionEditorPinDto {
    pub id: Box<str>,
    pub name: Box<str>,
    pub data_type: DataType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionEditorProjectionDto {
    pub function_revision: crate::project::ResourceRevision,
    pub inputs: Box<[FunctionEditorPinDto]>,
    pub outputs: Box<[FunctionEditorPinDto]>,
}

pub fn build_function_editor_projection(
    function: &crate::node_system::document::FunctionDocument,
) -> Result<FunctionEditorProjectionDto, String> {
    let inputs = function
        .signature
        .parameters
        .iter()
        .map(|parameter| {
            Ok(FunctionEditorPinDto {
                id: parameter.id.as_str().into(),
                name: parameter.name.clone().into_boxed_str(),
                data_type: resolve_function_data_type(&parameter.type_name)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_boxed_slice();
    let outputs = function
        .signature
        .return_type
        .as_deref()
        .map(|return_type| {
            Ok(FunctionEditorPinDto {
                id: "return".into(),
                name: return_type.into(),
                data_type: resolve_function_data_type(return_type)?,
            })
        })
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?
        .into_boxed_slice();
    Ok(FunctionEditorProjectionDto {
        function_revision: function.revision,
        inputs,
        outputs,
    })
}

pub(crate) fn resolve_function_data_type(type_name: &str) -> Result<DataType, String> {
    let data_type = type_name
        .parse()
        .map_err(|error: DataTypeParseError| error.to_string())?;
    validate_function_data_type(&data_type)?;
    Ok(data_type)
}

fn validate_function_data_type(data_type: &DataType) -> Result<(), String> {
    match data_type {
        DataType::Struct(key) if key.trim().is_empty() => {
            Err("Function Struct type key must not be empty".into())
        }
        DataType::Array(inner) | DataType::DataSeries(inner) => validate_function_data_type(inner),
        DataType::OneOf(inner) => inner.iter().try_for_each(validate_function_data_type),
        _ => Ok(()),
    }
}
