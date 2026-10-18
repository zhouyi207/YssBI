use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

use serde_json::Value;

use crate::{
    AgentEvent, CapabilityFailure, HarnessSessionId, HarnessTurnId, IdempotencyKey, PrincipalId,
    ProjectSessionBinding, ToolEffect, ToolInvocationId, UnixMillis,
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
    TurnConfigured {
        options: crate::HarnessTurnOptions,
    },
    AgentRunResumed {
        request: crate::AgentFollowup,
        scope: Value,
        role: crate::AgentRoleId,
        objective: String,
    },
    AgentRunStarted {
        run_id: crate::AgentRunId,
        parent_run_id: Option<crate::AgentRunId>,
        role: crate::AgentRoleId,
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
    pub project: ProjectSessionBinding,
    pub tool: String,
    pub effect: ToolEffect,
    /// Normalized model-facing arguments replayed to the provider. Malformed payloads are not retained.
    pub arguments: Value,
    /// The extension-bound request. None when the call was rejected before binding.
    pub request: Option<Value>,
    pub state: ToolInvocationState,
    pub result: Option<Value>,
    pub failure: Option<CapabilityFailure>,
    pub started_at: UnixMillis,
    pub deadline: UnixMillis,
    pub finished_at: Option<UnixMillis>,
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

    /// Atomically remove the session and its turns, events and invocations. Project
    /// resources are retained. Missing sessions return NotFound; the caller must exclude
    /// active work before deleting.
    fn delete_session<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
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
