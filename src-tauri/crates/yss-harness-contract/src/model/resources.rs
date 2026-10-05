//! Resource operations exposed to the model; owners retain concurrency contracts.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "operation",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ManageResourceInput {
    Create {
        specification: ResourceCreation,
    },
    Rename {
        resource: ProjectResourceRef,
        name: String,
    },
    Duplicate {
        resource: ProjectResourceRef,
    },
    Delete {
        resource: ProjectResourceRef,
    },
    Save {
        resource: ProjectResourceRef,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditResourceInput {
    pub resource: ProjectResourceRef,
    pub edit: ResourceEditInput,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResourceEditInput {
    Chart { settings: ChartSettings },
    Mind { operations: Vec<MindOperation> },
    Doc { operations: Vec<MarkdownOperation> },
    Database { operation: DatasetOperation },
    FunctionSignature { signature: FunctionSignatureInput },
    GraphHistory { redo: bool },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionSignatureInput {
    pub parameters: Vec<FunctionParameterInspection>,
    pub return_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportDatasetInput {
    pub resource: ProjectResourceRef,
    pub path: String,
    pub format: DatasetExportFormat,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestUiIntentInput {
    pub intent: yss_ui_contract::UiIntent,
}

impl From<&ManageResourceRequest> for ManageResourceInput {
    fn from(value: &ManageResourceRequest) -> Self {
        match value {
            ManageResourceRequest::Create { specification } => Self::Create {
                specification: specification.clone(),
            },
            ManageResourceRequest::Rename { resource, name, .. } => Self::Rename {
                resource: resource.clone(),
                name: name.clone(),
            },
            ManageResourceRequest::Duplicate { resource, .. } => Self::Duplicate {
                resource: resource.clone(),
            },
            ManageResourceRequest::Delete { resource, .. } => Self::Delete {
                resource: resource.clone(),
            },
            ManageResourceRequest::Save { resource, .. } => Self::Save {
                resource: resource.clone(),
            },
        }
    }
}

impl From<&FunctionSignatureInspection> for FunctionSignatureInput {
    fn from(value: &FunctionSignatureInspection) -> Self {
        Self {
            parameters: value.parameters.clone(),
            return_type: value.return_type.clone(),
        }
    }
}

impl From<&ResourceEdit> for ResourceEditInput {
    fn from(value: &ResourceEdit) -> Self {
        match value {
            ResourceEdit::Chart { settings } => Self::Chart {
                settings: settings.clone(),
            },
            ResourceEdit::Mind { operations } => Self::Mind {
                operations: operations.clone(),
            },
            ResourceEdit::Doc { operations } => Self::Doc {
                operations: operations.clone(),
            },
            ResourceEdit::Database { operation } => Self::Database {
                operation: operation.clone(),
            },
            ResourceEdit::FunctionSignature { signature } => Self::FunctionSignature {
                signature: signature.into(),
            },
            ResourceEdit::GraphHistory { redo, .. } => Self::GraphHistory { redo: *redo },
        }
    }
}
