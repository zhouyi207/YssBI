use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use schemars::{JsonSchema, Schema};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    ApprovalPolicy, AutomationCapabilityRequest, AutomationCapabilityResult,
    AutomationIdentityError, CapabilityFailure, CapabilityId, HarnessSessionId, PrincipalId,
    ProjectSessionBinding, StatisticalPlan, ToolEffect, capability_input_schema,
    capability_output_schema,
};

macro_rules! string_identity {
    ($name:ident, $label:literal) => {
        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, AutomationIdentityError> {
                let value = value.into();
                if value.trim().is_empty() || value.len() > 128 {
                    return Err(AutomationIdentityError::Invalid($label));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::try_new(value).map_err(serde::de::Error::custom)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct HarnessTurnId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct WorkflowId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct WorkflowVersion(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct WorkflowRunId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct WorkflowStepId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ToolId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ToolVersion(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ToolInvocationId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct IdempotencyKey(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct SkillId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct SkillVersion(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct MemoryRecordId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct KnowledgeSourceId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct KnowledgeDocumentId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct KnowledgeChunkId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct SourceHash(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct StatisticalMethodId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct MethodVersion(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ApprovalGrantId(String);

string_identity!(HarnessTurnId, "harness turn id");
string_identity!(WorkflowId, "workflow id");
string_identity!(WorkflowVersion, "workflow version");
string_identity!(WorkflowRunId, "workflow run id");
string_identity!(WorkflowStepId, "workflow step id");
string_identity!(ToolId, "tool id");
string_identity!(ToolVersion, "tool version");
string_identity!(ToolInvocationId, "tool invocation id");
string_identity!(IdempotencyKey, "idempotency key");
string_identity!(SkillId, "skill id");
string_identity!(SkillVersion, "skill version");
string_identity!(MemoryRecordId, "memory record id");
string_identity!(KnowledgeSourceId, "knowledge source id");
string_identity!(KnowledgeDocumentId, "knowledge document id");
string_identity!(KnowledgeChunkId, "knowledge chunk id");
string_identity!(SourceHash, "source hash");
string_identity!(StatisticalMethodId, "statistical method id");
string_identity!(MethodVersion, "method version");
string_identity!(ApprovalGrantId, "approval grant id");

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnixMillis(u64);

impl UnixMillis {
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, milliseconds: u64) -> Option<Self> {
        self.0.checked_add(milliseconds).map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataAccessPolicy {
    MetadataOnly,
    AggregatesOnly,
    BoundedRecords,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyPolicy {
    ReadOnlyReplayable,
    InvocationBound,
    ReconciliationRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultBudget {
    pub maximum_items: u16,
    pub maximum_bytes: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDescriptor {
    pub id: ToolId,
    pub version: ToolVersion,
    pub capability_id: CapabilityId,
    pub input_schema: Schema,
    pub output_schema: Schema,
    pub effect: ToolEffect,
    pub approval: ApprovalPolicy,
    pub data_access: DataAccessPolicy,
    pub timeout_ms: u64,
    pub idempotency: IdempotencyPolicy,
    pub result_budget: ResultBudget,
}

impl ToolDescriptor {
    pub fn for_capability(capability_id: CapabilityId) -> Result<Self, AutomationIdentityError> {
        let capability = capability_id.descriptor();
        Ok(Self {
            id: ToolId::try_new(capability_id.as_str())?,
            version: ToolVersion::try_new("1.0.0")?,
            capability_id,
            input_schema: capability_input_schema(capability_id),
            output_schema: capability_output_schema(capability_id),
            effect: capability.effect,
            approval: capability.approval,
            data_access: if capability.effect == ToolEffect::Mutate {
                DataAccessPolicy::BoundedRecords
            } else {
                DataAccessPolicy::MetadataOnly
            },
            timeout_ms: 30_000,
            idempotency: if capability.effect == ToolEffect::Mutate {
                IdempotencyPolicy::InvocationBound
            } else {
                IdempotencyPolicy::ReadOnlyReplayable
            },
            result_budget: ResultBudget {
                maximum_items: capability.maximum_results,
                maximum_bytes: 1_048_576,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentMessage {
    pub role: AgentMessageRole,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTurnRequest {
    pub session_id: HarnessSessionId,
    pub turn_id: HarnessTurnId,
    pub principal_id: PrincipalId,
    pub project: ProjectSessionBinding,
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<ToolDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTurnResult {
    pub final_text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelCapabilityRequest {
    pub request: AutomationCapabilityRequest,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelCapabilityOutcome {
    pub invocation_id: ToolInvocationId,
    pub result: AutomationCapabilityResult,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AgentEvent {
    TextDelta {
        delta: String,
    },
    PlanProposed {
        plan: StatisticalPlan,
    },
    ToolInvocationRequested {
        capability_id: CapabilityId,
    },
    ToolInvocationStarted {
        invocation_id: ToolInvocationId,
        capability_id: CapabilityId,
    },
    ToolInvocationCompleted {
        invocation_id: ToolInvocationId,
        capability_id: CapabilityId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum AgentDriverFailureCode {
    #[error("cancelled")]
    Cancelled,
    #[error("provider_unavailable")]
    ProviderUnavailable,
    #[error("invalid_provider_response")]
    InvalidProviderResponse,
    #[error("output_unavailable")]
    OutputUnavailable,
    #[error("internal_failure")]
    InternalFailure,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{code}")]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDriverFailure {
    pub code: AgentDriverFailureCode,
}

impl AgentDriverFailure {
    pub const fn new(code: AgentDriverFailureCode) -> Self {
        Self { code }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum AgentOutputFailure {
    #[error("output_closed")]
    Closed,
    #[error("output_persistence_failed")]
    PersistenceFailed,
    #[error("output_policy_rejected")]
    PolicyRejected,
}

pub type AgentFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait ModelCapabilityExecutor: Send + Sync {
    fn execute<'a>(
        &'a self,
        request: ModelCapabilityRequest,
    ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>>;
}

pub trait AgentEventOutput: Send + Sync {
    fn emit<'a>(&'a self, event: AgentEvent) -> AgentFuture<'a, Result<(), AgentOutputFailure>>;
}

pub trait AgentDriverPort: Send + Sync {
    fn run_turn<'a>(
        &'a self,
        request: AgentTurnRequest,
        capabilities: Arc<dyn ModelCapabilityExecutor>,
        output: Arc<dyn AgentEventOutput>,
        cancellation: CancellationToken,
    ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AgentDriverConfigurationFailure {
    #[error("agent provider configuration is invalid")]
    Invalid,
}

pub trait AgentDriverConfigurationPort: Send + Sync {
    fn configure(
        &self,
        base_url: String,
        model: String,
        credential: Option<SecretCredential>,
    ) -> Result<bool, AgentDriverConfigurationFailure>;

    fn is_configured(&self) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum CancellationReason {
    User = 1,
    SessionClosing = 2,
    ProjectReplaced = 3,
    DeadlineElapsed = 4,
}

#[derive(Debug, Default)]
struct CancellationState {
    reason: AtomicU8,
    waiter: Mutex<Option<Waker>>,
}

#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    state: Arc<CancellationState>,
}

impl CancellationToken {
    pub fn cancel(&self, reason: CancellationReason) -> bool {
        let cancelled = self
            .state
            .reason
            .compare_exchange(0, reason as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
        if cancelled
            && let Some(waiter) = self
                .state
                .waiter
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
        {
            waiter.wake();
        }
        cancelled
    }

    pub fn reason(&self) -> Option<CancellationReason> {
        match self.state.reason.load(Ordering::Acquire) {
            1 => Some(CancellationReason::User),
            2 => Some(CancellationReason::SessionClosing),
            3 => Some(CancellationReason::ProjectReplaced),
            4 => Some(CancellationReason::DeadlineElapsed),
            _ => None,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.reason().is_some()
    }

    pub fn cancelled(&self) -> CancellationFuture {
        CancellationFuture {
            token: self.clone(),
        }
    }
}

pub struct CancellationFuture {
    token: CancellationToken,
}

impl Future for CancellationFuture {
    type Output = CancellationReason;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(reason) = self.token.reason() {
            return Poll::Ready(reason);
        }
        *self
            .token
            .state
            .waiter
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(context.waker().clone());
        match self.token.reason() {
            Some(reason) => Poll::Ready(reason),
            None => Poll::Pending,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutomationIdKind {
    HarnessSession,
    HarnessTurn,
    WorkflowRun,
    ToolInvocation,
    CapabilityInvocation,
    MemoryRecord,
    ApprovalGrant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum IdGenerationFailure {
    #[error("id generation unavailable")]
    Unavailable,
}

pub trait IdGeneratorPort: Send + Sync {
    fn next_id(&self, kind: AutomationIdKind) -> Result<String, IdGenerationFailure>;
}

pub trait ClockPort: Send + Sync {
    fn now(&self) -> UnixMillis;
}

pub struct SecretCredential(Box<str>);

impl SecretCredential {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, CredentialFailure> {
        let value = value.into();
        if value.trim().is_empty() || value.len() > 16 * 1024 {
            return Err(CredentialFailure::Invalid);
        }
        Ok(Self(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretCredential([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CredentialFailure {
    #[error("credential is unavailable")]
    Unavailable,
    #[error("credential is invalid")]
    Invalid,
}
