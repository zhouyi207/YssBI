//! The domain extension seam. Harness owns turn lifecycle, delegation, durability and replay;
//! an extension owns tools, roles, authority and every interpretation of requests, results and
//! task scopes. Harness never inspects these JSON values beyond storing and forwarding them.

use std::any::Any;
use std::sync::Arc;

use serde_json::Value;

use crate::{
    AgentDriverFailure, AgentFuture, AgentProfile, AgentRoleId, AgentRunId, AgentTask,
    AgentTaskInput, AgentTaskOutcome, CapabilityControl, CapabilityFailure, HarnessMode,
    ProjectSessionBinding, ToolCall, ToolInvocationContext, ToolInvocationOutcome,
    ToolInvocationRecord,
};

/// A call bound by the extension. `request` is persisted and must be deterministic for retries.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedToolCall {
    pub tool: String,
    /// Normalized model-facing arguments retained for provider replay.
    pub arguments: Value,
    pub request: Value,
    /// A stable caller token. Equal tokens within one run share one durable invocation.
    pub idempotency_token: Option<String>,
    /// Issued by the extension itself rather than requested by the model.
    pub internal: bool,
}

/// A model call refused before binding. Only arguments the extension could decode are retained.
#[derive(Clone, Debug, PartialEq)]
pub struct RejectedToolCall {
    pub failure: CapabilityFailure,
    pub arguments: Option<Value>,
}

impl From<CapabilityFailure> for RejectedToolCall {
    fn from(failure: CapabilityFailure) -> Self {
        Self {
            failure,
            arguments: None,
        }
    }
}

/// An extension-defined fact published as `AgentEvent::Custom` for the settled invocation.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentNotice {
    pub kind: String,
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolExecution {
    pub result: Value,
    pub notices: Vec<AgentNotice>,
}

impl ToolExecution {
    pub fn new(result: Value) -> Self {
        Self {
            result,
            notices: Vec::new(),
        }
    }
}

/// The owner's answer when a previous invocation's outcome is unknown.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolRecovery {
    /// The request has no owner-level receipt lookup.
    Unsupported,
    /// The owner cannot prove that the request committed.
    Unknown,
    Committed(Value),
}

/// Runs extension-bound internal calls through the current run's durable lifecycle.
pub trait ToolRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        call: PreparedToolCall,
    ) -> AgentFuture<'a, Result<ToolInvocationOutcome, CapabilityFailure>>;

    /// A fresh opaque key for a request that the owner deduplicates by caller identity.
    fn new_call_key(&self) -> Result<String, CapabilityFailure>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunStart {
    pub run_id: AgentRunId,
    pub role: AgentRoleId,
    /// None for the root role, which has session-wide discovery.
    pub task_scope: Option<Value>,
    pub project: ProjectSessionBinding,
}

/// A task delegated in the current turn, offered to the root role's delivery check.
pub struct DelegatedTask<'a> {
    pub run_id: &'a AgentRunId,
    pub task: &'a AgentTask,
    pub outcome: Option<&'a AgentTaskOutcome>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunFinish {
    pub blocked_reason: Option<String>,
    pub next_steps: Vec<String>,
    pub facts: Value,
}

/// Per-run extension state. Harness creates one per agent run and feeds it every receipt.
pub trait AgentRunHooks: Send + Sync {
    fn as_any(&self) -> &dyn Any;

    /// Rebuild state from a recorded invocation of this run or, for the root, the session.
    fn replay(&self, record: &ToolInvocationRecord, project: &ProjectSessionBinding);

    /// A delegated run changed resources; drop observations its facts make stale.
    fn invalidate(&self, facts: &Value);

    /// Restore the authority recorded when this run was last resumed, before replaying later receipts.
    fn resume(&self, scope: &Value);

    /// System messages prepended when the run starts.
    fn context_messages(&self) -> Vec<String> {
        Vec::new()
    }

    /// Decode, authorize and bind a model call. Internal reads go through `runner`.
    fn prepare<'a>(
        &'a self,
        call: ToolCall,
        runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<PreparedToolCall, RejectedToolCall>>;

    /// Execute a bound call within the open ledger entry, including result checks.
    fn execute<'a>(
        &'a self,
        context: ToolInvocationContext,
        call: &'a PreparedToolCall,
        control: CapabilityControl,
    ) -> AgentFuture<'a, Result<ToolExecution, CapabilityFailure>>;

    /// Record the settled outcome of every call executed by this run.
    fn observe(
        &self,
        call: &PreparedToolCall,
        outcome: &Result<ToolInvocationOutcome, CapabilityFailure>,
    );

    /// Bind a root delegation to observed facts and return the worker task's authority.
    fn bind_task<'a>(
        &'a self,
        _input: AgentTaskInput,
        _runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<Value, CapabilityFailure>> {
        Box::pin(async {
            Err(CapabilityFailure::new(
                crate::CapabilityFailureCode::INVALID_REQUEST,
            ))
        })
    }

    /// Rebind a resumed worker within its original authority. Returns its scope and bindings.
    fn resume_task<'a>(
        &'a self,
        _worker: &'a dyn AgentRunHooks,
        _runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<(Value, Value), CapabilityFailure>> {
        Box::pin(async {
            Err(CapabilityFailure::new(
                crate::CapabilityFailureCode::INVALID_REQUEST,
            ))
        })
    }

    /// Verify a worker's inputs before its model starts. Some(text) blocks the run with that reply.
    fn before_run<'a>(
        &'a self,
        _runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<Option<String>, AgentDriverFailure>> {
        Box::pin(async { Ok(None) })
    }

    fn completion_feedback(&self, delegated: &[DelegatedTask<'_>]) -> Option<String>;

    fn finish(&self) -> RunFinish;
}

pub trait HarnessExtension: Send + Sync {
    /// The role that receives user turns and may delegate.
    fn root_role(&self) -> AgentRoleId;

    fn profile(&self, role: &AgentRoleId) -> Option<AgentProfile>;

    /// Extra instructions for the selected mode, prepended to every run.
    fn mode_instructions(&self, _mode: HarnessMode) -> Option<String> {
        None
    }

    /// The complete input schema of the root role's delegation tool.
    fn delegate_task_schema(&self) -> Value;

    /// Decode delegation arguments into their normalized form. Equal tasks have equal values.
    fn decode_task_input(&self, arguments: Value) -> Result<AgentTaskInput, CapabilityFailure>;

    fn validate_task(&self, task: &AgentTask) -> Result<(), CapabilityFailure>;

    fn scope_is_read_only(&self, scope: &Value) -> bool;

    /// The model-facing form of a bound task scope, used to replay delegation calls.
    fn task_scope_input(&self, scope: &Value) -> Value;

    /// Whether a completed task bound to `scope` is stale after another run produced `facts`.
    fn task_invalidated(&self, scope: &Value, facts: &Value) -> bool;

    fn project_result(&self, tool: &str, result: &Value) -> Result<Value, CapabilityFailure>;

    fn project_failure(&self, failure: &CapabilityFailure) -> Value;

    fn project_task_facts(&self, facts: &Value) -> Value;

    /// See `AgentToolExecutor::summary_projection`.
    fn summary_projection(&self, _tool: &str, _output: &Value) -> Option<Value> {
        None
    }

    /// Execute a pre-bound request outside an agent run.
    fn execute<'a>(
        &'a self,
        context: ToolInvocationContext,
        tool: &'a str,
        request: &'a Value,
        control: CapabilityControl,
    ) -> AgentFuture<'a, Result<ToolExecution, CapabilityFailure>>;

    fn recover<'a>(
        &'a self,
        _context: ToolInvocationContext,
        _tool: &'a str,
        _request: &'a Value,
    ) -> AgentFuture<'a, Result<ToolRecovery, CapabilityFailure>> {
        Box::pin(async { Ok(ToolRecovery::Unsupported) })
    }

    fn start_run(&self, run: RunStart) -> Arc<dyn AgentRunHooks>;
}
