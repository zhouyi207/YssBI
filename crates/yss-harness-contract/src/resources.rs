use crate::{CapabilityContractError, validate_resource_id};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub use yss_chart_document::ChartType;
pub use yss_project_identity::{ProjectResourceKind, ProjectResourceRef};

/// An explicit user-selected target. Contents remain with their resource owner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarnessResourceReference {
    pub resource: ProjectResourceRef,
    pub name: String,
}

impl HarnessResourceReference {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("resource.id", &self.resource.id)
    }
}

/// Resolve explicit references through the project owner during turn preparation.
pub trait HarnessResourceResolverPort: Send + Sync {
    fn resolve<'a>(
        &'a self,
        project: &'a crate::ProjectSessionBinding,
        resources: &'a [ProjectResourceRef],
        cancellation: crate::CancellationToken,
    ) -> crate::AgentFuture<'a, Result<Vec<HarnessResourceReference>, crate::CapabilityFailure>>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceVersion {
    pub revision: u64,
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectResourceRequest {
    pub resource: ProjectResourceRef,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "operation",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ManageResourceRequest {
    Create {
        specification: ResourceCreation,
    },
    Rename {
        resource: ProjectResourceRef,
        version: ResourceVersion,
        name: String,
    },
    Duplicate {
        resource: ProjectResourceRef,
        version: ResourceVersion,
        name: Option<String>,
    },
    Delete {
        resource: ProjectResourceRef,
        version: ResourceVersion,
    },
    Save {
        resource: ProjectResourceRef,
        version: ResourceVersion,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResourceCreation {
    EventGraph {
        name: String,
    },
    FunctionGraph {
        name: String,
    },
    Chart {
        name: String,
    },
    Mind {
        name: String,
    },
    Doc {
        name: String,
    },
    Database {
        source: DatasetImportSource,
        #[serde(default)]
        name: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DatasetImportSource {
    Csv {
        path: String,
        delimiter: char,
        has_header: bool,
        infer_schema_length: Option<usize>,
    },
    Parquet {
        path: String,
        columns: Option<Vec<String>>,
    },
    Excel {
        path: String,
        sheet: String,
    },
    Sql {
        engine: DatasetSqlEngine,
        connection_string: String,
        table: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DatasetSqlEngine {
    Sqlite { auto_create: bool },
    Postgres { ssl: bool },
    Mysql { charset: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DatasetExportFormat {
    Csv,
    Parquet,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportDatabaseRequest {
    pub resource: ProjectResourceRef,
    pub version: ResourceVersion,
    pub path: String,
    pub format: DatasetExportFormat,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditResourceRequest {
    pub resource: ProjectResourceRef,
    pub version: ResourceVersion,
    pub edit: ResourceEdit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResourceEdit {
    CreateColumns {
        columns: Vec<crate::model::DatabaseColumnDeclaration>,
    },
    RenameColumns {
        columns: Vec<crate::model::DatabaseColumnRename>,
    },
    DeleteColumns {
        columns: Vec<String>,
    },
    CastColumns {
        columns: Vec<crate::model::DatabaseColumnCast>,
    },
    SetColumnSemantics {
        columns: Vec<crate::model::DatabaseColumnMeaning>,
    },

    InsertRows {
        rows: Vec<BTreeMap<String, yss_data_contract::TabularScalar>>,
        before_row_id: Option<i64>,
    },
    UpdateCells {
        cells: Vec<crate::model::DatabaseCellEdit>,
    },
    DeleteRows {
        row_ids: Vec<i64>,
    },
    UpdateChart {
        settings: crate::model::ChartSettingsUpdate,
    },
    CreateTopics {
        topics: Vec<crate::model::TopicCreation>,
        before_id: Option<String>,
    },
    UpdateTopics {
        topics: Vec<crate::model::TopicUpdate>,
    },
    MoveTopics {
        topics: Vec<crate::model::TopicMove>,
    },
    DeleteTopics {
        topic_ids: Vec<String>,
    },
    DuplicateTopics {
        topic_ids: Vec<String>,
        parent_id: String,
        before_id: Option<String>,
    },
    ReplaceDocumentText {
        replacements: Vec<crate::model::DocumentTextReplacement>,
    },
    AppendDocument {
        text: String,
    },
    WriteDocument {
        markdown: String,
    },

    FunctionSignature {
        signature: FunctionSignatureInspection,
    },
    GraphHistory {
        redo: bool,
        graph_hash: String,
    },
    DatabaseHistory {
        redo: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartSettings {
    pub database_id: String,
    pub chart_type: ChartType,
    pub x: Option<String>,
    pub y: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum MindResourceReference {
    Resource { path: String },
    GraphNode { path: String, node_id: String },
    Database { database_id: String },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetColumnSemantic {
    pub kind: DatasetSemanticKind,
    pub values: Vec<DatasetSemanticValue>,
    pub positive_value: Option<String>,
    pub numeric: Option<DatasetNumericConstraints>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum DatasetSemanticKind {
    Numeric,
    Categorical,
    Ordinal,
    Binary,
    Datetime,
    Text,
    Identifier,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetSemanticValue {
    pub value: String,
    pub label: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetNumericConstraints {
    pub integer: bool,
    pub minimum: Option<String>,
    pub maximum: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionParameterInspection {
    pub id: Option<String>,
    pub name: String,
    pub type_name: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionSignatureInspection {
    pub revision: u64,
    pub parameters: Vec<FunctionParameterInspection>,
    pub return_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceInspection {
    pub resource: ProjectResourceRef,
    pub name: String,
    pub version: ResourceVersion,
    pub dirty: bool,
    pub content: ResourceContent,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResourceContent {
    Metadata,
    /// Existing function signature capability, without graph node or connection content.
    Function {
        signature: FunctionSignatureInspection,
    },
    /// Captured owner counters for binding older dataset observations; never model-visible.
    DatabaseMetadata {
        runtime_revision: u64,
        schema_revision: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceChange {
    pub resource: ProjectResourceRef,
    pub revision: u64,
    pub revision_kind: ResourceRevisionKind,
    pub deleted: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResourceRevisionKind {
    Resource,
    FunctionSignature,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceMove {
    pub from: ProjectResourceRef,
    pub to: ProjectResourceRef,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceMutationReceipt {
    pub publication_revision: Option<u64>,
    pub changes: Vec<ResourceChange>,
    pub moves: Vec<ResourceMove>,
    pub mind_edit: Option<crate::MindEditReceipt>,
    pub document_edit: Option<crate::DocumentEditReceipt>,
    /// Owner metadata verified at the committed version; absent if a concurrent change won.
    pub resources: Vec<ResourceMutationState>,
    pub database_edit: Option<DatabaseEditReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseEditReceipt {
    pub item_count: usize,
    pub inserted_row_ids: Vec<i64>,
    /// Committed target names (new names after rename; removed names after delete).
    pub column_names: Vec<String>,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceMutationState {
    pub resource: ProjectResourceRef,
    pub name: String,
    pub version: ResourceVersion,
    pub dirty: Option<bool>,
    pub root_topic_id: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseExported {
    pub resource: ProjectResourceRef,
    pub path: String,
}

impl InspectResourceRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("resource.id", &self.resource.id)
    }
}
impl ManageResourceRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        match self {
            Self::Create { specification } => match specification {
                ResourceCreation::EventGraph { name }
                | ResourceCreation::FunctionGraph { name }
                | ResourceCreation::Chart { name }
                | ResourceCreation::Mind { name }
                | ResourceCreation::Doc { name } => validate_resource_id("name", name),
                ResourceCreation::Database { source, name } => {
                    if let Some(name) = name {
                        validate_resource_id("name", name)?;
                    }
                    source.validate()
                }
            },
            Self::Rename { resource, name, .. } => {
                validate_resource_id("name", name)?;
                validate_resource_id("resource.id", &resource.id)
            }
            Self::Duplicate { resource, name, .. } => {
                if let Some(name) = name {
                    validate_resource_id("name", name)?;
                }
                validate_resource_id("resource.id", &resource.id)
            }
            Self::Delete { resource, .. } | Self::Save { resource, .. } => {
                validate_resource_id("resource.id", &resource.id)
            }
        }
    }
}
impl DatasetImportSource {
    fn validate(&self) -> Result<(), CapabilityContractError> {
        match self {
            Self::Csv { path, .. } | Self::Parquet { path, .. } | Self::Excel { path, .. } => {
                validate_resource_id("path", path)
            }
            Self::Sql {
                connection_string,
                table,
                ..
            } => {
                validate_resource_id("connectionString", connection_string)?;
                validate_resource_id("table", table)
            }
        }
    }
}
impl EditResourceRequest {
    pub const fn capability_id(&self) -> crate::CapabilityId {
        match &self.edit {
            ResourceEdit::UpdateChart { .. } => crate::CapabilityId::UpdateChart,
            ResourceEdit::ReplaceDocumentText { .. } => crate::CapabilityId::ReplaceDocumentText,
            ResourceEdit::AppendDocument { .. } => crate::CapabilityId::AppendDocument,
            ResourceEdit::WriteDocument { .. } => crate::CapabilityId::WriteDocument,

            ResourceEdit::CreateTopics { .. } => crate::CapabilityId::CreateTopics,
            ResourceEdit::UpdateTopics { .. } => crate::CapabilityId::UpdateTopics,
            ResourceEdit::MoveTopics { .. } => crate::CapabilityId::MoveTopics,
            ResourceEdit::DeleteTopics { .. } => crate::CapabilityId::DeleteTopics,
            ResourceEdit::DuplicateTopics { .. } => crate::CapabilityId::DuplicateTopics,

            ResourceEdit::CreateColumns { .. } => crate::CapabilityId::CreateColumns,
            ResourceEdit::RenameColumns { .. } => crate::CapabilityId::RenameColumns,
            ResourceEdit::DeleteColumns { .. } => crate::CapabilityId::DeleteColumns,
            ResourceEdit::CastColumns { .. } => crate::CapabilityId::CastColumns,
            ResourceEdit::SetColumnSemantics { .. } => crate::CapabilityId::SetColumnSemantics,
            ResourceEdit::InsertRows { .. } => crate::CapabilityId::InsertRows,
            ResourceEdit::UpdateCells { .. } => crate::CapabilityId::UpdateCells,
            ResourceEdit::DeleteRows { .. } => crate::CapabilityId::DeleteRows,
            ResourceEdit::GraphHistory { redo: false, .. }
            | ResourceEdit::DatabaseHistory { redo: false } => crate::CapabilityId::UndoResource,
            ResourceEdit::GraphHistory { redo: true, .. }
            | ResourceEdit::DatabaseHistory { redo: true } => crate::CapabilityId::RedoResource,
            _ => crate::CapabilityId::EditResource,
        }
    }

    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("resource.id", &self.resource.id)?;
        let valid = matches!(
            (&self.resource.kind, &self.edit),
            (ProjectResourceKind::Chart, ResourceEdit::UpdateChart { .. })
                | (
                    ProjectResourceKind::Mind,
                    ResourceEdit::CreateTopics { .. }
                        | ResourceEdit::UpdateTopics { .. }
                        | ResourceEdit::MoveTopics { .. }
                        | ResourceEdit::DeleteTopics { .. }
                        | ResourceEdit::DuplicateTopics { .. }
                )
                | (
                    ProjectResourceKind::Doc,
                    ResourceEdit::ReplaceDocumentText { .. }
                        | ResourceEdit::AppendDocument { .. }
                        | ResourceEdit::WriteDocument { .. }
                )
                | (
                    ProjectResourceKind::Database,
                    ResourceEdit::InsertRows { .. }
                        | ResourceEdit::UpdateCells { .. }
                        | ResourceEdit::DeleteRows { .. }
                        | ResourceEdit::CreateColumns { .. }
                        | ResourceEdit::RenameColumns { .. }
                        | ResourceEdit::DeleteColumns { .. }
                        | ResourceEdit::CastColumns { .. }
                        | ResourceEdit::SetColumnSemantics { .. }
                )
                | (
                    ProjectResourceKind::Database,
                    ResourceEdit::DatabaseHistory { .. }
                )
                | (
                    ProjectResourceKind::FunctionGraph,
                    ResourceEdit::FunctionSignature { .. }
                )
                | (
                    ProjectResourceKind::EventGraph | ProjectResourceKind::FunctionGraph,
                    ResourceEdit::GraphHistory { .. }
                )
        );
        if !valid {
            return Err(CapabilityContractError::InvalidField("edit.kind"));
        }
        let count = match &self.edit {
            ResourceEdit::UpdateChart { settings } => {
                settings.validate()?;
                1
            }
            ResourceEdit::InsertRows {
                rows,
                before_row_id,
            } => {
                if before_row_id.is_some_and(|id| id < 0) {
                    return Err(CapabilityContractError::InvalidField("beforeRowId"));
                }
                for row in rows {
                    for column in row.keys() {
                        validate_resource_id("rows.column", column)?;
                    }
                }
                rows.len()
            }
            ResourceEdit::UpdateCells { cells } => {
                crate::model::validate_cells(cells)?;
                cells.len()
            }
            ResourceEdit::DeleteRows { row_ids } => {
                crate::model::validate_row_ids(row_ids)?;
                row_ids.len()
            }
            ResourceEdit::CreateColumns { columns } => {
                crate::model::validate_column_names(
                    columns.iter().map(|column| column.name.as_str()),
                )?;
                for column in columns {
                    validate_resource_id("columns.dtype", &column.dtype)?;
                }
                columns.len()
            }
            ResourceEdit::RenameColumns { columns } => {
                crate::model::validate_column_names(
                    columns.iter().map(|column| column.column.as_str()),
                )?;
                crate::model::validate_column_names(
                    columns.iter().map(|column| column.name.as_str()),
                )?;
                columns.len()
            }
            ResourceEdit::DeleteColumns { columns } => {
                crate::model::validate_column_names(columns.iter().map(String::as_str))?;
                columns.len()
            }
            ResourceEdit::CastColumns { columns } => {
                crate::model::validate_column_names(
                    columns.iter().map(|column| column.column.as_str()),
                )?;
                for column in columns {
                    validate_resource_id("columns.dtype", &column.dtype)?;
                }
                columns.len()
            }
            ResourceEdit::SetColumnSemantics { columns } => {
                crate::model::validate_column_names(
                    columns.iter().map(|column| column.column.as_str()),
                )?;
                columns.len()
            }
            ResourceEdit::CreateTopics { .. }
            | ResourceEdit::UpdateTopics { .. }
            | ResourceEdit::MoveTopics { .. }
            | ResourceEdit::DeleteTopics { .. }
            | ResourceEdit::DuplicateTopics { .. } => {
                crate::model::validate_topic_edits(&self.edit)?
            }
            ResourceEdit::ReplaceDocumentText { replacements } => replacements.len(),
            _ => 1,
        };
        if count == 0 || count > 200 {
            return Err(CapabilityContractError::InvalidLimit { maximum: 200 });
        }
        // Markdown body size is enforced by the original Doc owner, not the graph JSON budget.
        if matches!(
            self.edit,
            ResourceEdit::ReplaceDocumentText { .. }
                | ResourceEdit::AppendDocument { .. }
                | ResourceEdit::WriteDocument { .. }
        ) {
            Ok(())
        } else {
            crate::graph::validate_graph_json(&self.edit)
        }
    }
}

impl ExportDatabaseRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        if self.resource.kind != ProjectResourceKind::Database {
            return Err(CapabilityContractError::InvalidField("resource.kind"));
        }
        validate_resource_id("resource.id", &self.resource.id)?;
        validate_resource_id("path", &self.path)
    }
}
