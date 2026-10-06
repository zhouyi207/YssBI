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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentFollowupInput {
    pub run_id: AgentRunId,
    pub instruction: String,
}

impl From<&AgentTask> for AgentTaskInput {
    fn from(task: &AgentTask) -> Self {
        Self {
            worker: task.worker,
            objective: task.objective.clone(),
            constraints: task.constraints.clone(),
            completion_criteria: task.completion_criteria.clone(),
            depends_on: task.depends_on.clone(),
            scope: AgentTaskScopeInput {
                resources: task
                    .scope
                    .resources
                    .iter()
                    .map(|access| AgentResourceInput {
                        resource: access.resource.clone(),
                        operations: access.operations.clone(),
                    })
                    .collect(),
                results: task.scope.results.clone(),
                creations: task.scope.creations.clone(),
                export_paths: task.scope.export_paths.clone(),
            },
        }
    }
}

impl From<&AgentFollowup> for AgentFollowupInput {
    fn from(request: &AgentFollowup) -> Self {
        Self {
            run_id: request.run_id.clone(),
            instruction: request.instruction.clone(),
        }
    }
}

pub fn task_outcome(outcome: &AgentTaskOutcome) -> serde_json::Value {
    serde_json::json!({
        "runId": outcome.run_id,
        "role": outcome.role,
        "state": outcome.state,
        "report": outcome.report,
        "failureCode": outcome.failure_code,
        "artifacts": outcome.artifacts.iter().map(|change| serde_json::json!({
            "resource": change.resource,
            "deleted": change.deleted,
        })).collect::<Vec<_>>(),
        "results": outcome.results,
        "evidence": outcome.evidence,
        "plan": outcome.plan,
        "invalidatedRuns": outcome.invalidated_runs,
    })
}
