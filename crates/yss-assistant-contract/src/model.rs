//! Explicit model projections; durable authority stays in the internal contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::*;

mod chart;
mod database;
mod document;
mod graph;
mod inputs;
mod lifecycle;
mod mind;
mod nodes;
mod resources;
mod results;
mod ui;
pub use chart::*;
pub use database::*;
pub use document::*;
pub use graph::*;
pub use inputs::*;
pub use lifecycle::*;
pub use mind::*;
pub use nodes::*;
pub use resources::*;
pub use results::{capability_result, failure, failure_code};
pub use ui::*;

/// The typed form of the Harness delegation arguments. The extension decodes it and publishes its schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTaskInput {
    pub worker: AgentRole,
    pub objective: String,
    /// Required top-level string describing the worker's constraints, such as read-only access or preserving source data.
    pub constraints: String,
    pub completion_criteria: String,
    pub depends_on: Vec<AgentRunId>,
    pub scope: AgentTaskScopeInput,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTaskScopeInput {
    pub resources: Vec<AgentResourceInput>,
    pub results: Vec<AgentResultAccess>,
    pub creations: Vec<AgentCreationAccess>,
    pub export_paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentResourceInput {
    pub resource: ProjectResourceRef,
    pub operations: Vec<AgentResourceOperation>,
}

impl From<&AgentTaskScope> for AgentTaskScopeInput {
    fn from(scope: &AgentTaskScope) -> Self {
        Self {
            resources: scope
                .resources
                .iter()
                .map(|access| AgentResourceInput {
                    resource: access.resource.clone(),
                    operations: access.operations.clone(),
                })
                .collect(),
            results: scope.results.clone(),
            creations: scope.creations.clone(),
            export_paths: scope.export_paths.clone(),
        }
    }
}

/// Model-facing task facts. Internal revisions stay out of the delegation result.
pub fn task_facts(facts: &AgentTaskFacts) -> serde_json::Value {
    serde_json::json!({
        "artifacts": facts.artifacts.iter().map(|change| serde_json::json!({
            "resource": change.resource,
            "deleted": change.deleted,
        })).collect::<Vec<_>>(),
        "results": facts.results,
        "plan": facts.plan,
    })
}
