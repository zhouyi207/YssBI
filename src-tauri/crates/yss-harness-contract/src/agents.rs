//! Flat Manager–Worker identities, delegation and execution authority.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    AgentDriverFailureCode, AgentRunId, CapabilityContractError, GraphResultReference,
    ProjectResourceRef, ResourceCreation, ResourceVersion, StatisticalPlan, ToolInvocationId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Manager,
    Data,
    Stats,
    Plot,
    Report,
    Review,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentControlTool {
    DelegateTask,
    ProposeStatisticalPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentOutputMode {
    Transcript,
    FinalResponse,
}

impl AgentRole {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Manager => "ManagerAgent",
            Self::Data => "DataAgent",
            Self::Stats => "StatsAgent",
            Self::Plot => "PlotAgent",
            Self::Report => "ReportAgent",
            Self::Review => "ReviewAgent",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentResourceOperation {
    Inspect,
    Edit,
    Rename,
    Duplicate,
    Delete,
    Save,
    Execute,
    Export,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentResourceAccess {
    pub resource: ProjectResourceRef,
    pub version: Option<ResourceVersion>,
    pub operations: Vec<AgentResourceOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentResultAccess {
    pub execution_session_id: String,
    pub result_id: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTaskScope {
    pub resources: Vec<AgentResourceAccess>,
    pub results: Vec<AgentResultAccess>,
    pub creations: Vec<AgentCreationAccess>,
    pub export_paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentCreationAccess {
    pub specification: ResourceCreation,
    pub operations: Vec<AgentResourceOperation>,
}

impl AgentTaskScope {
    pub fn is_read_only(&self) -> bool {
        self.creations.is_empty()
            && self.export_paths.is_empty()
            && self.resources.iter().all(|access| {
                access
                    .operations
                    .iter()
                    .all(|op| *op == AgentResourceOperation::Inspect)
            })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTask {
    /// Stable within the user turn. Reusing a key never starts the work twice.
    pub key: String,
    pub worker: AgentRole,
    pub objective: String,
    pub constraints: String,
    pub completion_criteria: String,
    pub depends_on: Vec<AgentRunId>,
    pub scope: AgentTaskScope,
}

impl AgentTask {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        if self.worker == AgentRole::Manager
            || self.key.trim().is_empty()
            || self.key.len() > 128
            || self.objective.trim().is_empty()
            || self.completion_criteria.trim().is_empty()
            || (self.worker == AgentRole::Review && !self.scope.is_read_only())
        {
            return Err(CapabilityContractError::InvalidField("task"));
        }
        for access in &self.scope.resources {
            crate::validate_resource_id("resourceId", &access.resource.id)?;
            if access.version.is_none() {
                return Err(CapabilityContractError::InvalidField(
                    "scope.resources.version",
                ));
            }
            if !access.operations.contains(&AgentResourceOperation::Inspect)
                || access.operations.len() > 8
            {
                return Err(CapabilityContractError::InvalidField("operations"));
            }
        }
        for creation in &self.scope.creations {
            crate::ManageResourceRequest::Create {
                specification: creation.specification.clone(),
            }
            .validate()?;
            if !creation
                .operations
                .contains(&AgentResourceOperation::Inspect)
                || creation.operations.len() > 8
            {
                return Err(CapabilityContractError::InvalidField("creation.operations"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentInvocationScope {
    pub run_id: AgentRunId,
    pub role: AgentRole,
    /// Only the Manager has project-wide discovery. Workers always receive a scope.
    pub task: Option<AgentTaskScope>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerReport {
    pub summary: String,
    pub warnings: Vec<String>,
    pub blocked_reason: Option<String>,
    pub next_steps: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunState {
    Completed,
    Stale,
    Blocked,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTaskOutcome {
    pub run_id: AgentRunId,
    pub role: AgentRole,
    pub state: AgentRunState,
    pub report: Option<WorkerReport>,
    pub failure_code: Option<AgentDriverFailureCode>,
    /// These facts are collected by Core from actual tool receipts, never from model prose.
    pub artifacts: Vec<crate::ResourceChange>,
    pub results: Vec<GraphResultReference>,
    pub evidence: Vec<ToolInvocationId>,
    pub plan: Option<StatisticalPlan>,
    pub invalidated_runs: Vec<AgentRunId>,
}

pub fn agent_task_schema() -> schemars::Schema {
    schemars::schema_for!(AgentTask)
}
