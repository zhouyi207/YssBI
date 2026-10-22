//! Manager–Worker identities, delegation and outcomes. Task scopes are extension-defined JSON.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::string_identity;
use crate::{AgentDriverFailureCode, AgentRunId, ToolInvocationId};

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct AgentRoleId(String);

string_identity!(AgentRoleId, "agent role id");

pub const DELEGATE_TASK_TOOL: &str = "agent_delegate_task";
pub const FOLLOWUP_TASK_TOOL: &str = "agent_continue_task";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentOutputMode {
    Transcript,
    FinalResponse,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentFollowup {
    pub run_id: AgentRunId,
    pub instruction: String,
    /// Extension facts bound when the run resumed, such as observed resource versions.
    pub bindings: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTask {
    /// Stable within the user turn. Reusing a key never starts the work twice.
    pub key: String,
    pub worker: AgentRoleId,
    pub objective: String,
    pub constraints: String,
    pub completion_criteria: String,
    pub depends_on: Vec<AgentRunId>,
    /// The bound authority granted by the extension's delegation hook.
    pub scope: Value,
}

impl AgentTask {
    pub fn has_valid_text(&self) -> bool {
        !self.key.trim().is_empty()
            && self.key.len() <= 128
            && !self.objective.trim().is_empty()
            && !self.completion_criteria.trim().is_empty()
    }
}

/// The model-facing delegation arguments. The extension owns the scope's schema and binding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTaskInput {
    pub worker: AgentRoleId,
    pub objective: String,
    pub constraints: String,
    pub completion_criteria: String,
    pub depends_on: Vec<AgentRunId>,
    pub scope: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentFollowupInput {
    pub run_id: AgentRunId,
    pub instruction: String,
}

pub fn followup_task_schema() -> Value {
    Value::from(schemars::schema_for!(AgentFollowupInput))
}

impl From<&AgentFollowup> for AgentFollowupInput {
    fn from(request: &AgentFollowup) -> Self {
        Self {
            run_id: request.run_id.clone(),
            instruction: request.instruction.clone(),
        }
    }
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
    pub role: AgentRoleId,
    pub state: AgentRunState,
    pub report: Option<WorkerReport>,
    pub failure_code: Option<AgentDriverFailureCode>,
    pub evidence: Vec<ToolInvocationId>,
    /// Extension facts collected from actual tool receipts, never from model prose.
    pub facts: Value,
    pub invalidated_runs: Vec<AgentRunId>,
}

/// One role's static definition. The root role receives user turns and may delegate.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentProfile {
    pub role: AgentRoleId,
    pub instructions: Vec<String>,
    pub tools: Vec<crate::ToolSpec>,
    pub tool_concurrency: usize,
    pub delegates: bool,
    pub output_mode: AgentOutputMode,
}
