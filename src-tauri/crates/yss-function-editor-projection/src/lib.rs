//! Canonical typed projection of a persisted function signature for editors.
//!
//! The projection keeps its project resource revision strongly typed and owns
//! the stable transport shape shared by project-index and mutation-event APIs.
//! Project I/O, editor state, and event delivery remain with their respective
//! owners.

use serde::{Deserialize, Serialize};
use yss_data_contract::{DataType, DataTypeParseError};
use yss_project_history::FunctionDocument;
use yss_project_identity::ResourceRevision;

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
    pub function_revision: ResourceRevision,
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

impl TryFrom<&FunctionDocument> for FunctionEditorProjection {
    type Error = FunctionEditorProjectionError;

    fn try_from(document: &FunctionDocument) -> Result<Self, Self::Error> {
        let inputs = document
            .signature
            .parameters
            .iter()
            .map(|parameter| {
                Ok(FunctionEditorPin {
                    id: parameter.id.as_str().into(),
                    name: parameter.name.as_str().into(),
                    data_type: parse_function_data_type(&parameter.type_name)?,
                })
            })
            .collect::<Result<Vec<_>, FunctionEditorProjectionError>>()?
            .into_boxed_slice();
        let outputs = document
            .signature
            .return_type
            .as_deref()
            .map(|return_type| {
                Ok(FunctionEditorPin {
                    id: "return".into(),
                    name: return_type.into(),
                    data_type: parse_function_data_type(return_type)?,
                })
            })
            .into_iter()
            .collect::<Result<Vec<_>, FunctionEditorProjectionError>>()?
            .into_boxed_slice();

        Ok(Self {
            function_revision: document.revision,
            inputs,
            outputs,
        })
    }
}

pub fn parse_function_data_type(
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use yss_graph_document::FunctionParameterId;
    use yss_project_history::{FunctionParameter, FunctionSignature};

    fn function_document(type_name: &str, return_type: Option<&str>) -> FunctionDocument {
        FunctionDocument {
            revision: ResourceRevision::new(7),
            signature: FunctionSignature {
                parameters: vec![FunctionParameter {
                    id: FunctionParameterId::new("value"),
                    name: "Value".to_owned(),
                    type_name: type_name.to_owned(),
                }],
                return_type: return_type.map(str::to_owned),
            },
        }
    }

    #[test]
    fn document_projection_has_one_typed_revision_and_stable_wire_shape() {
        let projection =
            FunctionEditorProjection::try_from(&function_document("Int32", Some("Float64")))
                .unwrap();

        assert_eq!(projection.function_revision, ResourceRevision::new(7));
        assert_eq!(projection.inputs[0].data_type, DataType::Int64);
        assert_eq!(
            serde_json::to_value(&projection).unwrap(),
            json!({
                "functionRevision": 7,
                "inputs": [{
                    "id": "value",
                    "name": "Value",
                    "dataType": { "kind": "Int64" }
                }],
                "outputs": [{
                    "id": "return",
                    "name": "Float64",
                    "dataType": { "kind": "Float64" }
                }]
            })
        );
    }

    #[test]
    fn nested_empty_struct_key_is_rejected() {
        assert_eq!(
            FunctionEditorProjection::try_from(&function_document("Array<Struct<   >>", None))
                .unwrap_err(),
            FunctionEditorProjectionError::EmptyStructType
        );
    }

    #[test]
    fn projection_wire_rejects_unknown_fields() {
        let value = json!({
            "functionRevision": 0,
            "inputs": [],
            "outputs": [],
            "legacy": true
        });
        assert!(serde_json::from_value::<FunctionEditorProjection>(value).is_err());
    }
}
