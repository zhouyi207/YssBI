use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

use crate::{
    AgentEvent, ApprovalGrantId, AutomationCapabilityRequest, AutomationCapabilityResult,
    CapabilityFailure, CapabilityId, HarnessSessionId, HarnessTurnId, IdempotencyKey, PrincipalId,
    ProjectSessionBinding, SourceHash, ToolInvocationId, UnixMillis, WorkflowId, WorkflowRunId,
    WorkflowStepId, WorkflowVersion,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessSessionState {
    Active,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessTurnState {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarnessSessionRecord {
    pub id: HarnessSessionId,
    pub principal_id: PrincipalId,
    pub project: ProjectSessionBinding,
    pub conversation: Option<HarnessConversationMetadata>,
    pub state: HarnessSessionState,
    pub created_at: UnixMillis,
    pub updated_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarnessConversationMetadata {
    /// Stable project root identity, independent of runtime activation IDs.
    pub project_key: String,
    pub title: String,
    pub last_opened_at: UnixMillis,
    pub model: Option<crate::LanguageModelSelection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarnessTurnRecord {
    pub id: HarnessTurnId,
    pub session_id: HarnessSessionId,
    pub state: HarnessTurnState,
    pub user_message: String,
    pub final_text: Option<String>,
    pub started_at: UnixMillis,
    pub finished_at: Option<UnixMillis>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum HarnessEvent {
    AgentRunResumed {
        request: crate::AgentFollowup,
        scope: crate::AgentTaskScope,
        role: crate::AgentRole,
        objective: String,
    },
    AgentRunStarted {
        run_id: crate::AgentRunId,
        parent_run_id: Option<crate::AgentRunId>,
        role: crate::AgentRole,
        task: Option<Box<crate::AgentTask>>,
    },
    AgentRunOutput {
        run_id: crate::AgentRunId,
        event: AgentEvent,
    },
    AgentRunFinished {
        outcome: Box<crate::AgentTaskOutcome>,
    },
    AgentRunInvalidated {
        run_id: crate::AgentRunId,
    },
    SessionCreated,
    TurnStarted {
        user_message: String,
        model: crate::LanguageModelIdentity,
        resources: Vec<crate::HarnessResourceReference>,
    },
    Agent(AgentEvent),
    TurnCompleted {
        final_text: String,
    },
    TurnFailed,
    TurnCancelled,
    KnowledgeCited {
        citation: crate::KnowledgeCitation,
    },
    WorkflowPlanned {
        run_id: WorkflowRunId,
    },
    WorkflowStarted {
        run_id: WorkflowRunId,
    },
    WorkflowStepStarted {
        run_id: WorkflowRunId,
        step_id: WorkflowStepId,
    },
    WorkflowStepCompleted {
        run_id: WorkflowRunId,
        step_id: WorkflowStepId,
    },
    WorkflowStepFailed {
        run_id: WorkflowRunId,
        step_id: WorkflowStepId,
        retriable: bool,
    },
    WorkflowCompleted {
        run_id: WorkflowRunId,
    },
    WorkflowPaused {
        run_id: WorkflowRunId,
    },
    WorkflowResumed {
        run_id: WorkflowRunId,
    },
    WorkflowCancelled {
        run_id: WorkflowRunId,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarnessEventEnvelope {
    pub sequence: u64,
    pub session_id: HarnessSessionId,
    pub turn_id: Option<HarnessTurnId>,
    pub occurred_at: UnixMillis,
    pub event: HarnessEvent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowDefinition {
    pub id: WorkflowId,
    pub version: WorkflowVersion,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowStep {
    pub id: WorkflowStepId,
    pub depends_on: Vec<WorkflowStepId>,
    pub request: AutomationCapabilityRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunState {
    Planned,
    Ready,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepState {
    Pending,
    Running,
    Succeeded,
    RetriableFailure,
    TerminalFailure,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowStepRecord {
    pub state: WorkflowStepState,
    pub attempt: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowRunRecord {
    pub id: WorkflowRunId,
    pub revision: u64,
    pub session_id: HarnessSessionId,
    pub turn_id: Option<HarnessTurnId>,
    pub definition_id: WorkflowId,
    pub definition_version: WorkflowVersion,
    pub project: ProjectSessionBinding,
    pub state: WorkflowRunState,
    pub steps: BTreeMap<WorkflowStepId, WorkflowStepRecord>,
    pub created_at: UnixMillis,
    pub updated_at: UnixMillis,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolInvocationState {
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolInvocationRecord {
    pub id: ToolInvocationId,
    pub idempotency_key: IdempotencyKey,
    pub session_id: HarnessSessionId,
    pub turn_id: HarnessTurnId,
    pub agent_run_id: Option<crate::AgentRunId>,
    pub workflow_run_id: Option<WorkflowRunId>,
    pub workflow_step_id: Option<WorkflowStepId>,
    pub project: ProjectSessionBinding,
    pub capability_id: CapabilityId,
    pub request: AutomationCapabilityRequest,
    pub state: ToolInvocationState,
    pub result: Option<AutomationCapabilityResult>,
    pub failure: Option<CapabilityFailure>,
    pub started_at: UnixMillis,
    pub deadline: UnixMillis,
    pub finished_at: Option<UnixMillis>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalGrantRecord {
    pub id: ApprovalGrantId,
    pub principal_id: PrincipalId,
    pub session_id: HarnessSessionId,
    pub project: ProjectSessionBinding,
    pub capability_id: CapabilityId,
    pub request_fingerprint: SourceHash,
    pub issued_at: UnixMillis,
    pub expires_at: UnixMillis,
    pub consumed_at: Option<UnixMillis>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ToolInvocationBegin {
    Started,
    Existing(Box<ToolInvocationRecord>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceFailureCode {
    #[error("conflict")]
    Conflict,
    #[error("not_found")]
    NotFound,
    #[error("unavailable")]
    Unavailable,
    #[error("invalid_record")]
    InvalidRecord,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{code}")]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistenceFailure {
    pub code: PersistenceFailureCode,
}

impl PersistenceFailure {
    pub const fn new(code: PersistenceFailureCode) -> Self {
        Self { code }
    }
}

pub type PersistenceFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait HarnessSessionStorePort: Send + Sync {
    fn list_conversations<'a>(
        &'a self,
        principal: &'a PrincipalId,
        project_key: &'a str,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessSessionRecord>, PersistenceFailure>>;

    fn load_running_turns<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessTurnRecord>, PersistenceFailure>>;

    fn create_session<'a>(
        &'a self,
        record: &'a HarnessSessionRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn load_session<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<Option<HarnessSessionRecord>, PersistenceFailure>>;

    fn load_active_sessions<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessSessionRecord>, PersistenceFailure>>;

    fn update_session<'a>(
        &'a self,
        record: &'a HarnessSessionRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn create_turn<'a>(
        &'a self,
        record: &'a HarnessTurnRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn load_turn<'a>(
        &'a self,
        turn_id: &'a HarnessTurnId,
    ) -> PersistenceFuture<'a, Result<Option<HarnessTurnRecord>, PersistenceFailure>>;

    fn update_turn<'a>(
        &'a self,
        record: &'a HarnessTurnRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;
}

pub trait HarnessEventStorePort: Send + Sync {
    /// Allocate the next session sequence and persist the envelope atomically.
    /// A failed append must not consume a sequence.
    fn append_event<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
        turn_id: Option<&'a HarnessTurnId>,
        occurred_at: UnixMillis,
        event: HarnessEvent,
    ) -> PersistenceFuture<'a, Result<HarnessEventEnvelope, PersistenceFailure>>;

    fn load_events_after<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
        sequence: u64,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessEventEnvelope>, PersistenceFailure>>;

    fn latest_sequence<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<u64, PersistenceFailure>>;
}

pub trait HarnessEventSinkPort: Send + Sync {
    fn publish<'a>(
        &'a self,
        event: &'a HarnessEventEnvelope,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;
}

pub trait WorkflowStorePort: Send + Sync {
    fn save_definition<'a>(
        &'a self,
        definition: &'a WorkflowDefinition,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn load_definition<'a>(
        &'a self,
        id: &'a WorkflowId,
        version: &'a WorkflowVersion,
    ) -> PersistenceFuture<'a, Result<Option<WorkflowDefinition>, PersistenceFailure>>;

    /// None inserts revision zero; Some(r) compares r and returns revision r + 1.
    /// A missing/changed row or duplicate creation returns Conflict.
    fn save_run<'a>(
        &'a self,
        run: &'a WorkflowRunRecord,
        expected_revision: Option<u64>,
    ) -> PersistenceFuture<'a, Result<WorkflowRunRecord, PersistenceFailure>>;

    fn load_run<'a>(
        &'a self,
        id: &'a WorkflowRunId,
    ) -> PersistenceFuture<'a, Result<Option<WorkflowRunRecord>, PersistenceFailure>>;

    fn load_recoverable_runs<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<WorkflowRunRecord>, PersistenceFailure>>;
}

pub trait ToolInvocationLedgerPort: Send + Sync {
    fn load_invocation<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
        invocation_id: &'a ToolInvocationId,
    ) -> PersistenceFuture<'a, Result<Option<ToolInvocationRecord>, PersistenceFailure>>;

    fn load_running_invocations<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<ToolInvocationRecord>, PersistenceFailure>>;

    fn begin<'a>(
        &'a self,
        record: &'a ToolInvocationRecord,
    ) -> PersistenceFuture<'a, Result<ToolInvocationBegin, PersistenceFailure>>;

    fn finish<'a>(
        &'a self,
        record: &'a ToolInvocationRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;
}

pub trait ApprovalStorePort: Send + Sync {
    fn insert<'a>(
        &'a self,
        record: &'a ApprovalGrantRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn load<'a>(
        &'a self,
        id: &'a ApprovalGrantId,
    ) -> PersistenceFuture<'a, Result<Option<ApprovalGrantRecord>, PersistenceFailure>>;

    fn consume<'a>(
        &'a self,
        id: &'a ApprovalGrantId,
        consumed_at: UnixMillis,
    ) -> PersistenceFuture<'a, Result<bool, PersistenceFailure>>;
}
