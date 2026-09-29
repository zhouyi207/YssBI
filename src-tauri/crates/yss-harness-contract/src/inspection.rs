//! Typed project, catalog, dataset and result inspection requests and facts.

use crate::{DatasetColumnSemantic, ProjectResourceRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchNodeCatalogRequest {
    pub query: String,
    pub locale: String,
    pub limit: u16,
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
pub struct InspectResultRequest {
    pub execution_session_id: String,
    pub result_id: u64,
    /// Table-reference part returned in result JSON. Omit to read the full JSON result.
    #[serde(default)]
    pub part: Option<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_result_page_limit")]
    pub limit: u16,
}

fn default_result_page_limit() -> u16 {
    20
}

#[derive(Clone, Debug, Default, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectProjectRequest {}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeCatalogMatch {
    pub node_type_id: String,
    pub title: String,
    pub category_id: String,
    pub style_id: String,
    pub resource_path: Option<String>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeCatalogSearchResult {
    pub locale: String,
    pub matches: Vec<NodeCatalogMatch>,
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
    Table {
        columns: Vec<String>,
        #[serde(rename = "columnTypes")]
        column_types: Vec<String>,
        rows: Vec<serde_json::Value>,
        #[serde(rename = "nextOffset")]
        next_offset: usize,
        #[serde(rename = "hasMore")]
        has_more: bool,
    },
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultInspection {
    pub result_id: u64,
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
}
