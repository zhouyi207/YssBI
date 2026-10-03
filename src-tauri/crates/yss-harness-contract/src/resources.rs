use crate::{
    CapabilityContractError, DatasetSchemaInspection, GraphInspection, validate_resource_id,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub use yss_chart_document::ChartType;
pub use yss_project_identity::{ProjectResourceKind, ProjectResourceRef};

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
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_page_size")]
    pub limit: usize,
}
fn default_page_size() -> usize {
    100
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
    EventGraph { name: String },
    FunctionGraph { name: String },
    Chart { name: String },
    Mind { name: String },
    Doc { name: String },
    Database { source: DatasetImportSource },
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
pub struct ExportDatasetRequest {
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
    Chart {
        settings: ChartSettings,
    },
    Mind {
        operations: Vec<MindOperation>,
    },
    Doc {
        operations: Vec<MarkdownOperation>,
    },
    Database {
        operation: DatasetOperation,
    },
    FunctionSignature {
        signature: FunctionSignatureInspection,
    },
    GraphHistory {
        redo: bool,
        graph_hash: String,
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
pub struct MindTopic {
    pub id: String,
    pub parent_id: Option<String>,
    pub content: String,
    pub reference: Option<MindResourceReference>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum MindOperation {
    AddNode {
        client_id: String,
        parent_id: String,
        content: String,
    },
    SetContent {
        node_id: String,
        content: String,
    },
    SetReference {
        node_id: String,
        reference: Option<MindResourceReference>,
    },
    MoveNode {
        node_id: String,
        parent_id: String,
        before_id: Option<String>,
    },
    RemoveNode {
        node_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum MarkdownOperation {
    SetMarkdown {
        markdown: String,
    },
    /// Replace [start, end) using zero-based Unicode scalar offsets (not UTF-8 bytes or
    /// UTF-16 units). Ranges apply after preceding operations; start == end inserts.
    /// Preserve text outside the range, including portions not returned by inspection.
    ReplaceRange {
        start: usize,
        end: usize,
        markdown: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DatasetOperation {
    EditCell {
        row: usize,
        column: String,
        value: serde_json::Value,
        row_id: Option<i64>,
    },
    AddRow {
        index: Option<usize>,
    },
    DeleteRows {
        indices: Vec<usize>,
        row_ids: Option<Vec<i64>>,
    },
    AddColumn {
        name: String,
        dtype: String,
    },
    DeleteColumn {
        name: String,
    },
    CastColumn {
        column: String,
        dtype: String,
        force: bool,
    },
    SetColumnSemantic {
        column: String,
        semantic: DatasetColumnSemantic,
    },
    RenameColumn {
        old_name: String,
        new_name: String,
    },
    Undo,
    Redo,
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
    Graph {
        graph: GraphInspection,
        function: Option<FunctionSignatureInspection>,
        can_undo: bool,
        can_redo: bool,
    },
    Chart {
        settings: ChartSettings,
    },
    Mind {
        root_id: String,
        nodes: Vec<MindTopic>,
        total_nodes: usize,
        next_offset: Option<usize>,
    },
    Doc {
        markdown: String,
        total_characters: usize,
        next_offset: Option<usize>,
    },
    Database {
        schema: DatasetSchemaInspection,
        rows: Vec<Vec<serde_json::Value>>,
        row_ids: Vec<i64>,
        next_offset: Option<usize>,
        can_undo: bool,
        can_redo: bool,
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
    pub created_nodes: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetExported {
    pub resource: ProjectResourceRef,
    pub path: String,
}

impl InspectResourceRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("resource.id", &self.resource.id)?;
        let maximum = match self.resource.kind {
            ProjectResourceKind::Doc => 16_384,
            ProjectResourceKind::Mind => 500,
            _ => 100,
        };
        if self.limit == 0 || self.limit > maximum {
            return Err(CapabilityContractError::InvalidLimit {
                maximum: maximum as u16,
            });
        }
        Ok(())
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
                ResourceCreation::Database { source } => source.validate(),
            },
            Self::Rename { resource, name, .. } => {
                validate_resource_id("name", name)?;
                validate_resource_id("resource.id", &resource.id)
            }
            Self::Duplicate { resource, .. }
            | Self::Delete { resource, .. }
            | Self::Save { resource, .. } => validate_resource_id("resource.id", &resource.id),
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
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("resource.id", &self.resource.id)?;
        let valid = matches!(
            (&self.resource.kind, &self.edit),
            (ProjectResourceKind::Chart, ResourceEdit::Chart { .. })
                | (ProjectResourceKind::Mind, ResourceEdit::Mind { .. })
                | (ProjectResourceKind::Doc, ResourceEdit::Doc { .. })
                | (ProjectResourceKind::Database, ResourceEdit::Database { .. })
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
            ResourceEdit::Mind { operations } => operations.len(),
            ResourceEdit::Doc { operations } => operations.len(),
            _ => 1,
        };
        if count == 0 || count > 200 {
            return Err(CapabilityContractError::InvalidLimit { maximum: 200 });
        }
        crate::graph::validate_graph_json(&self.edit)
    }
}

impl ExportDatasetRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        if self.resource.kind != ProjectResourceKind::Database {
            return Err(CapabilityContractError::InvalidField("resource.kind"));
        }
        validate_resource_id("resource.id", &self.resource.id)?;
        validate_resource_id("path", &self.path)
    }
}
