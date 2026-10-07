//! Typed project, catalog, dataset and result inspection requests and facts.

use crate::{DatasetColumnSemantic, ProjectResourceRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowseNodesRequest {
    #[serde(default)]
    pub query: String,
    pub locale: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_catalog_page_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: u16,
}

fn default_catalog_page_limit() -> u16 {
    20
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectNodeTypeRequest {
    /// Exact type IDs returned by browse_nodes; one definition per requested type.
    #[schemars(length(min = 1, max = 100))]
    pub type_ids: Vec<String>,
    pub locale: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectDatasetSchemaRequest {
    pub database_id: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectDatasetProfileRequest {
    pub database_id: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListResourcesRequest {
    #[serde(default)]
    pub kinds: Vec<crate::ProjectResourceKind>,
    /// Case-insensitive name or path substring; does not search resource contents.
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_resource_page_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
}

fn default_resource_page_limit() -> usize {
    100
}

impl Default for ListResourcesRequest {
    fn default() -> Self {
        Self {
            kinds: vec![],
            query: None,
            offset: 0,
            limit: default_resource_page_limit(),
        }
    }
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectionPage {
    pub offset: usize,
    pub returned: usize,
    pub total: Option<usize>,
    pub has_more: bool,
    pub next_offset: Option<usize>,
}

impl InspectionPage {
    pub fn known(offset: usize, returned: usize, total: usize) -> Self {
        let end = offset.saturating_add(returned);
        Self {
            offset,
            returned,
            total: Some(total),
            has_more: end < total,
            next_offset: (end < total).then_some(end),
        }
    }
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeCatalogMatch {
    pub type_id: String,
    pub title: String,
    pub category_id: String,
    pub summary: Option<String>,
    pub resource_path: Option<String>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeTypeDefinition {
    pub type_id: String,
    pub title: String,
    pub documentation: Option<String>,
    /// Schema for the parameters and portCounts maps accepted at creation.
    pub configuration_schema: serde_json::Value,
    pub ports: Vec<NodeCreationPortDefinition>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeTypeInspection {
    pub locale: String,
    pub types: Vec<NodeTypeDefinition>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeCreationPortDefinition {
    pub key: String,
    pub title: String,
    pub direction: String,
    pub count: NodePortCountPolicy,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NodePortCountPolicy {
    Fixed,
    Configurable {
        min: u16,
        max: Option<u16>,
        member_templates: Vec<String>,
    },
    Derived,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeCatalogPage {
    pub locale: String,
    pub matches: Vec<NodeCatalogMatch>,
    pub page: InspectionPage,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetColumnSchema {
    pub name: String,
    pub data_type: String,
    pub physical_type: String,
    pub semantic: Option<DatasetColumnSemantic>,
    pub nullable: bool,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetSchemaInspection {
    pub database_id: String,
    pub runtime_revision: u64,
    pub schema_revision: u64,
    pub columns: Vec<DatasetColumnSchema>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetProfileInspection {
    pub database_id: String,
    pub runtime_revision: u64,
    pub schema_revision: u64,
    pub row_count: usize,
    pub column_count: usize,
    pub estimated_memory_bytes: Option<usize>,
    pub duplicated_rows: Option<usize>,
    pub numeric_columns: usize,
    pub categorical_columns: usize,
    pub string_columns: usize,
    pub temporal_columns: usize,
    pub boolean_columns: usize,
    pub total_nulls: usize,
    pub null_ratio: f64,
    pub columns_with_nulls: usize,
    pub rows_with_nulls: usize,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResultCategoryInspection {
    Value,
    PlotData { plot_kind: String },
    StatisticalReport { report_kind: String },
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ResultValueInspection {
    Json(serde_json::Value),
    Tabular {
        #[serde(rename = "tableRef")]
        table_ref: crate::TableRef,
        columns: Vec<crate::ResultColumn>,
        #[serde(rename = "schemaPage")]
        schema_page: crate::InspectionPage,
        #[serde(rename = "rowCount")]
        row_count: Option<usize>,
        #[serde(rename = "schemaKnown")]
        schema_known: bool,
    },
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultInspection {
    pub result_ref: crate::ResultRef,
    pub validity: crate::ResultValidity,
    pub category: ResultCategoryInspection,
    pub value: ResultValueInspection,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectResourceInspection {
    pub resource: ProjectResourceRef,
    pub display_name: String,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectInspection {
    pub project_name: String,
    pub publication_revision: u64,
    pub resources: Vec<ProjectResourceInspection>,
    pub page: InspectionPage,
}
