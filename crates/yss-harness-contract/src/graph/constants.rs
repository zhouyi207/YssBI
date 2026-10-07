//! Constant business values reuse the neutral data contract, never resource JSON.
use super::*;
use crate::{InspectionPage, model::NodePositionInput};
use yss_data_contract::{DataValue, TabularScalar, TabularSnapshot, ValueType};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstantValueInput {
    pub data_type: ValueType,
    #[serde(default)]
    pub data_value: DataValue,
    #[serde(default)]
    pub tabular: Option<TabularSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstantReferenceNode {
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub position: NodePositionInput,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstantDeclaration {
    pub client_id: String,
    pub name: String,
    pub value: ConstantValueInput,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub reference_node: Option<ConstantReferenceNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstantUpdate {
    pub constant_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub value: Option<ConstantValueInput>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateConstantsInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub constants: Vec<ConstantDeclaration>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateConstantsInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub constants: Vec<ConstantUpdate>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteConstantsInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub constant_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindConstantsRequest {
    pub graph: GraphResourceRef,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "constant_page_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConstantValuePath {
    Field { key: String },
    Item { index: usize },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectConstantsRequest {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 100))]
    pub constant_ids: Vec<String>,
    /// Navigate nested DataValue objects/lists before paging. Empty reads the root.
    #[serde(default)]
    pub value_path: Vec<ConstantValuePath>,
    /// Unicode character, list item, object field, byte or table row offset.
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "constant_page_limit")]
    #[schemars(range(min = 1, max = 16384))]
    pub limit: usize,
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub column_offset: usize,
    #[serde(default = "constant_page_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub column_limit: usize,
}

fn constant_page_limit() -> usize {
    50
}

impl FindConstantsRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("graph", &self.graph.id)?;
        if self.limit == 0 || self.limit > 100 {
            return Err(CapabilityContractError::InvalidField("limit"));
        }
        if self.query.as_ref().is_some_and(|query| query.len() > 2048) {
            return Err(CapabilityContractError::InvalidField("query"));
        }
        Ok(())
    }
}

impl InspectConstantsRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("graph", &self.graph.id)?;
        if self.constant_ids.is_empty() || self.constant_ids.len() > 100 {
            return Err(CapabilityContractError::InvalidField("constantIds"));
        }
        if self.limit == 0 || self.limit > 16384 {
            return Err(CapabilityContractError::InvalidField("limit"));
        }
        if self.column_limit == 0 || self.column_limit > 100 {
            return Err(CapabilityContractError::InvalidField("columnLimit"));
        }
        if self.columns.len() > 100 {
            return Err(CapabilityContractError::InvalidField("columns"));
        }
        if self.value_path.len() > 64 {
            return Err(CapabilityContractError::InvalidField("valuePath"));
        }
        for id in &self.constant_ids {
            validate_resource_id("constantIds", id)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstantSummary {
    pub constant_id: String,
    pub name: String,
    pub data_type: ValueType,
    pub preview: String,
    pub row_count: Option<usize>,
    pub column_count: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ConstantReadValue {
    Value {
        value: DataValue,
        page: InspectionPage,
    },
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<TabularScalar>>,
        page: InspectionPage,
        column_page: InspectionPage,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstantInspection {
    pub summary: ConstantSummary,
    pub description: String,
    pub tags: Vec<String>,
    pub value_path: Vec<ConstantValuePath>,
    pub value: ConstantReadValue,
}
