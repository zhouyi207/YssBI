//! Graph intentions without edit identities or conditional-read tokens.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectGraphInput {
    pub graph_path: String,
    #[serde(default)]
    pub view: GraphInspectionView,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub port_addresses: Vec<GraphEditPortRef>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub include_schema: bool,
    #[serde(default)]
    pub include_options: bool,
}

fn default_limit() -> usize {
    50
}

impl From<&InspectGraphRequest> for InspectGraphInput {
    fn from(value: &InspectGraphRequest) -> Self {
        Self {
            graph_path: value.graph_path.clone(),
            view: value.view,
            node_ids: value.node_ids.clone(),
            port_addresses: value.port_addresses.clone(),
            offset: value.offset,
            limit: value.limit,
            include_schema: value.include_schema,
            include_options: value.include_options,
        }
    }
}

impl From<InspectGraphInput> for InspectGraphRequest {
    fn from(value: InspectGraphInput) -> Self {
        Self {
            graph_path: value.graph_path,
            view: value.view,
            node_ids: value.node_ids,
            port_addresses: value.port_addresses,
            offset: value.offset,
            limit: value.limit,
            include_schema: value.include_schema,
            include_options: value.include_options,
            if_unchanged: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyGraphEditInput {
    pub graph_path: String,
    pub locale: String,
    pub operations: Vec<GraphEditOperation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphTargetInput {
    pub graph_path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecuteGraphInput {
    pub graph_path: String,
    pub demand: GraphExecutionDemand,
}
