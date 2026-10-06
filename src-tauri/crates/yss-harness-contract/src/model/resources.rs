//! Resource operations exposed to the model; owners retain concurrency contracts.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectResourceInput {
    pub resource: ProjectResourceRef,
}
impl From<InspectResourceInput> for InspectResourceRequest {
    fn from(value: InspectResourceInput) -> Self {
        Self {
            resource: value.resource,
        }
    }
}
impl From<&InspectResourceRequest> for InspectResourceInput {
    fn from(value: &InspectResourceRequest) -> Self {
        Self {
            resource: value.resource.clone(),
        }
    }
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
    FunctionSignature { signature: FunctionSignatureInput },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionSignatureInput {
    pub parameters: Vec<FunctionParameterInspection>,
    pub return_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportDatabaseInput {
    pub database: super::DatabaseResourceRef,
    pub path: String,
    pub format: DatasetExportFormat,
}

impl From<&FunctionSignatureInspection> for FunctionSignatureInput {
    fn from(value: &FunctionSignatureInspection) -> Self {
        Self {
            parameters: value.parameters.clone(),
            return_type: value.return_type.clone(),
        }
    }
}

impl From<&EditResourceRequest> for super::CapabilityInput {
    fn from(value: &EditResourceRequest) -> Self {
        let target = || super::ResourceTargetInput {
            resource: value.resource.clone(),
        };
        let edit = match &value.edit {
            ResourceEdit::InsertRows {
                rows,
                before_row_id,
            } => {
                return Self::InsertRows(super::InsertRowsInput {
                    database: super::DatabaseResourceRef::new(value.resource.id.clone()),
                    rows: rows.clone(),
                    before_row_id: *before_row_id,
                });
            }
            ResourceEdit::UpdateCells { cells } => {
                return Self::UpdateCells(super::UpdateCellsInput {
                    database: super::DatabaseResourceRef::new(value.resource.id.clone()),
                    cells: cells.clone(),
                });
            }
            ResourceEdit::DeleteRows { row_ids } => {
                return Self::DeleteRows(super::DeleteRowsInput {
                    database: super::DatabaseResourceRef::new(value.resource.id.clone()),
                    row_ids: row_ids.clone(),
                });
            }
            ResourceEdit::CreateColumns { columns } => {
                return Self::CreateColumns(super::CreateColumnsInput {
                    database: super::DatabaseResourceRef::new(value.resource.id.clone()),
                    columns: columns.clone(),
                });
            }
            ResourceEdit::RenameColumns { columns } => {
                return Self::RenameColumns(super::RenameColumnsInput {
                    database: super::DatabaseResourceRef::new(value.resource.id.clone()),
                    columns: columns.clone(),
                });
            }
            ResourceEdit::DeleteColumns { columns } => {
                return Self::DeleteColumns(super::DeleteColumnsInput {
                    database: super::DatabaseResourceRef::new(value.resource.id.clone()),
                    columns: columns.clone(),
                });
            }
            ResourceEdit::CastColumns { columns } => {
                return Self::CastColumns(super::CastColumnsInput {
                    database: super::DatabaseResourceRef::new(value.resource.id.clone()),
                    columns: columns.clone(),
                });
            }
            ResourceEdit::SetColumnSemantics { columns } => {
                return Self::SetColumnSemantics(super::SetColumnSemanticsInput {
                    database: super::DatabaseResourceRef::new(value.resource.id.clone()),
                    columns: columns.clone(),
                });
            }
            ResourceEdit::GraphHistory { redo: false, .. }
            | ResourceEdit::DatabaseHistory { redo: false } => return Self::UndoResource(target()),
            ResourceEdit::GraphHistory { redo: true, .. }
            | ResourceEdit::DatabaseHistory { redo: true } => return Self::RedoResource(target()),
            ResourceEdit::UpdateChart { settings } => {
                return Self::UpdateChart(super::UpdateChartInput {
                    chart: super::ChartResourceRef::new(value.resource.id.clone()),
                    settings: settings.clone(),
                });
            }
            ResourceEdit::CreateTopics { topics, before_id } => {
                return Self::CreateTopics(super::CreateTopicsInput {
                    mind: super::MindResourceRef::new(value.resource.id.clone()),
                    topics: topics.clone(),
                    before_id: before_id.clone(),
                });
            }
            ResourceEdit::UpdateTopics { topics } => {
                return Self::UpdateTopics(super::UpdateTopicsInput {
                    mind: super::MindResourceRef::new(value.resource.id.clone()),
                    topics: topics.clone(),
                });
            }
            ResourceEdit::MoveTopics { topics } => {
                return Self::MoveTopics(super::MoveTopicsInput {
                    mind: super::MindResourceRef::new(value.resource.id.clone()),
                    topics: topics.clone(),
                });
            }
            ResourceEdit::DeleteTopics { topic_ids } => {
                return Self::DeleteTopics(super::DeleteTopicsInput {
                    mind: super::MindResourceRef::new(value.resource.id.clone()),
                    topic_ids: topic_ids.clone(),
                });
            }
            ResourceEdit::DuplicateTopics {
                topic_ids,
                parent_id,
                before_id,
            } => {
                return Self::DuplicateTopics(super::DuplicateTopicsInput {
                    mind: super::MindResourceRef::new(value.resource.id.clone()),
                    topic_ids: topic_ids.clone(),
                    parent_id: parent_id.clone(),
                    before_id: before_id.clone(),
                });
            }
            ResourceEdit::ReplaceDocumentText { replacements } => {
                return Self::ReplaceDocumentText(super::ReplaceDocumentTextInput {
                    document: super::DocumentResourceRef::new(value.resource.id.clone()),
                    replacements: replacements.clone(),
                });
            }
            ResourceEdit::AppendDocument { text } => {
                return Self::AppendDocument(super::AppendDocumentInput {
                    document: super::DocumentResourceRef::new(value.resource.id.clone()),
                    text: text.clone(),
                });
            }
            ResourceEdit::WriteDocument { markdown } => {
                return Self::WriteDocument(super::WriteDocumentInput {
                    document: super::DocumentResourceRef::new(value.resource.id.clone()),
                    markdown: markdown.clone(),
                });
            }
            ResourceEdit::FunctionSignature { signature } => ResourceEditInput::FunctionSignature {
                signature: signature.into(),
            },
        };
        Self::EditResource(EditResourceInput {
            resource: value.resource.clone(),
            edit,
        })
    }
}
