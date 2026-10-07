//! Graph intentions without edit identities or conditional-read tokens.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectGraphInput {
    pub graph: GraphResourceRef,
}

impl From<&InspectGraphRequest> for InspectGraphInput {
    fn from(value: &InspectGraphRequest) -> Self {
        Self {
            graph: value.graph.clone(),
        }
    }
}

impl From<InspectGraphInput> for InspectGraphRequest {
    fn from(value: InspectGraphInput) -> Self {
        Self::summary(value.graph)
    }
}

// Read projection for a stored internal operation; no callable model schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyGraphEditInput {
    pub graph_path: String,
    pub locale: String,
    pub operations: Vec<GraphEditOperation>,
}

// Read projection for a stored internal operation; no callable model schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphTargetInput {
    pub graph_path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecuteGraphInput {
    pub graph: GraphResourceRef,
    #[serde(default)]
    pub node_id: Option<String>,
    /// Required only to override the default dependencies mode for a selected node.
    #[serde(default)]
    pub mode: Option<NodeExecutionMode>,
}

impl ExecuteGraphInput {
    pub fn demand(&self) -> Result<GraphExecutionDemand, CapabilityContractError> {
        match &self.node_id {
            Some(node_id) => Ok(GraphExecutionDemand::Node {
                node_id: node_id.clone(),
                mode: self.mode.clone().unwrap_or(NodeExecutionMode::Dependencies),
            }),
            None if self.mode.is_none() => Ok(GraphExecutionDemand::Default),
            None => Err(CapabilityContractError::InvalidField("mode")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidateGraphInput {
    pub graph: GraphResourceRef,
    #[serde(default)]
    #[schemars(length(max = 200))]
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_diagnostic_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
}

fn default_diagnostic_limit() -> usize {
    50
}
