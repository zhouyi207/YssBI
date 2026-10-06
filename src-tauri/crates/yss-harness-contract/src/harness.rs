use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use schemars::{JsonSchema, Schema};
use serde::{Deserialize, Serialize};

use crate::context::string_identity;
use crate::{
    AutomationCapabilityResult, CapabilityFailure, CapabilityFailureCode, CapabilityId,
    StatisticalPlan, capability_input_schema,
};

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct HarnessTurnId(String);

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct AgentRunId(String);

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
string_identity!(AgentRunId, "agent run id");
string_identity!(WorkflowId, "workflow id");
string_identity!(WorkflowVersion, "workflow version");
string_identity!(WorkflowRunId, "workflow run id");
string_identity!(WorkflowStepId, "workflow step id");
string_identity!(ToolInvocationId, "tool invocation id");
string_identity!(IdempotencyKey, "idempotency key");
string_identity!(SkillId, "skill id");
string_identity!(SkillVersion, "skill version");
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDescriptor {
    pub capability_id: CapabilityId,
    pub input_schema: Schema,
}

impl ToolDescriptor {
    pub fn for_capability(capability_id: CapabilityId) -> Self {
        Self {
            capability_id,
            input_schema: capability_input_schema(capability_id),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentMessage {
    CompactionCheckpoint {
        checkpoint: ContextCompactionCheckpoint,
    },
    DelegationCall {
        run_id: AgentRunId,
        task: crate::AgentTask,
    },
    DelegationResult {
        outcome: Box<crate::AgentTaskOutcome>,
    },
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: String,
    },
    ToolCall {
        invocation_id: crate::ToolInvocationId,
        request: crate::ToolInvocationRequest,
    },
    ToolResult {
        invocation_id: crate::ToolInvocationId,
        capability_id: CapabilityId,
        outcome: Result<AutomationCapabilityResult, CapabilityFailure>,
    },
    Plan {
        plan: StatisticalPlan,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTurnRequest {
    pub options: crate::HarnessTurnOptions,
    pub role: crate::AgentRole,
    pub tool_concurrency: usize,
    pub control_tools: Vec<crate::AgentControlTool>,
    pub output_mode: crate::AgentOutputMode,
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
    pub request: crate::model::CapabilityInput,
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
    ReasoningDelta {
        delta: String,
    },
    UsageReported {
        usage: crate::ModelTokenUsage,
        context_window: Option<u32>,
        purpose: crate::ModelCallPurpose,
    },
    KnowledgeCited {
        citation: crate::KnowledgeCitation,
    },
    ContextCompactionProgress {
        completed_bytes: usize,
        total_bytes: usize,
        checkpoint: Option<ContextCompactionCheckpoint>,
    },
    GraphExecutionFinished {
        invocation_id: ToolInvocationId,
        status: String,
        failure_code: Option<String>,
    },
    TextRetracted {
        characters: usize,
    },
    ContextCompacted {
        summary: String,
    },
    RuntimeStatus {
        phase: AgentRuntimePhase,
        attempt: u32,
    },
    DeliveryBlocked {
        reason: String,
    },
    TextDelta {
        delta: String,
    },
    PlanProposed {
        plan: StatisticalPlan,
    },
    ControlToolStarted {
        invocation_id: ToolInvocationId,
        tool: crate::AgentControlTool,
    },
    ControlToolFinished {
        invocation_id: ToolInvocationId,
        tool: crate::AgentControlTool,
        failure_code: Option<crate::CapabilityFailureCode>,
        failure_details: Option<std::collections::BTreeMap<String, String>>,
    },
    ToolInvocationStarted {
        invocation_id: ToolInvocationId,
        capability_id: CapabilityId,
    },
    ToolInvocationCompleted {
        invocation_id: ToolInvocationId,
        capability_id: CapabilityId,
    },
    ToolInvocationFailed {
        invocation_id: ToolInvocationId,
        capability_id: CapabilityId,
        failure_code: crate::CapabilityFailureCode,
        failure_details: Option<std::collections::BTreeMap<String, String>>,
    },
}

/// A resumable summary of an exact source prefix, never a replacement for unread history.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextCompactionCheckpoint {
    pub processed_bytes: usize,
    pub prefix_hash: String,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRuntimePhase {
    Compacting,
    Reconnecting,
    CheckingDelivery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum AgentDriverFailureCode {
    #[error("cancelled")]
    Cancelled,
    #[error("provider_unavailable")]
    ProviderUnavailable,
    #[error("provider_authentication_failed")]
    ProviderAuthenticationFailed,
    #[error("provider_rate_limited")]
    ProviderRateLimited,
    #[error("provider_request_rejected")]
    ProviderRequestRejected,
    #[error("context_window_exceeded")]
    ContextWindowExceeded,
    #[error("provider_output_truncated")]
    ProviderOutputTruncated,
    #[error("provider_stream_interrupted")]
    ProviderStreamInterrupted,
    #[error("provider_content_filtered")]
    ProviderContentFiltered,
    #[error("provider_payment_required")]
    ProviderPaymentRequired,
    #[error("provider_transport_failed")]
    ProviderTransportFailed,
    #[error("deadline_elapsed")]
    DeadlineElapsed,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl AgentDriverFailure {
    pub const fn new(code: AgentDriverFailureCode) -> Self {
        Self {
            code,
            retry_after_ms: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum AgentOutputFailure {
    #[error("output_closed")]
    Closed,
    #[error("output_persistence_failed")]
    PersistenceFailed,
    #[error("output_policy_rejected")]
    PolicyRejected {
        reason: String,
        available_methods: Vec<crate::StatisticalMethodCard>,
    },
}

pub type AgentFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait ModelCapabilityExecutor: Send + Sync {
    /// Core owns control-call identity and durable timing; adapters supply no clock.
    fn begin_control<'a>(
        &'a self,
        _tool: crate::AgentControlTool,
    ) -> AgentFuture<'a, Result<ToolInvocationId, CapabilityFailure>> {
        Box::pin(async {
            Err(CapabilityFailure::new(
                CapabilityFailureCode::InternalFailure,
            ))
        })
    }

    fn finish_control<'a>(
        &'a self,
        _invocation_id: ToolInvocationId,
        _tool: crate::AgentControlTool,
        _failure: Option<CapabilityFailure>,
    ) -> AgentFuture<'a, Result<(), CapabilityFailure>> {
        Box::pin(async {
            Err(CapabilityFailure::new(
                CapabilityFailureCode::InternalFailure,
            ))
        })
    }

    fn execute<'a>(
        &'a self,
        request: ModelCapabilityRequest,
    ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>>;

    /// Report a provider argument failure through the host-owned invocation lifecycle.
    fn reject_arguments<'a>(
        &'a self,
        _capability_id: CapabilityId,
        failure: CapabilityFailure,
    ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
        Box::pin(async move { Err(failure) })
    }

    fn delegate<'a>(
        &'a self,
        _task: crate::model::AgentTaskInput,
    ) -> AgentFuture<'a, Result<crate::AgentTaskOutcome, CapabilityFailure>> {
        Box::pin(async {
            Err(CapabilityFailure::new(
                crate::CapabilityFailureCode::InvalidRequest,
            ))
        })
    }

    fn followup<'a>(
        &'a self,
        _request: crate::model::AgentFollowupInput,
    ) -> AgentFuture<'a, Result<crate::AgentTaskOutcome, CapabilityFailure>> {
        Box::pin(async {
            Err(CapabilityFailure::new(
                crate::CapabilityFailureCode::InvalidRequest,
            ))
        })
    }

    /// Core's authoritative delivery check. Rig only feeds this back to the model.
    fn completion_feedback(&self) -> Option<String> {
        None
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum CancellationReason {
    User = 1,
    ProjectReplaced = 2,
    DeadlineElapsed = 3,
}

#[derive(Debug, Default)]
struct CancellationState {
    reason: AtomicU8,
    next_waiter: AtomicUsize,
    waiters: Mutex<BTreeMap<usize, Waker>>,
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
        if cancelled {
            let waiters = std::mem::take(
                &mut *self
                    .state
                    .waiters
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()),
            );
            for waiter in waiters.into_values() {
                waiter.wake();
            }
        }
        cancelled
    }

    pub fn reason(&self) -> Option<CancellationReason> {
        match self.state.reason.load(Ordering::Acquire) {
            1 => Some(CancellationReason::User),
            2 => Some(CancellationReason::ProjectReplaced),
            3 => Some(CancellationReason::DeadlineElapsed),
            _ => None,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.reason().is_some()
    }

    pub fn cancelled(&self) -> CancellationFuture {
        CancellationFuture {
            token: self.clone(),
            waiter_id: self.state.next_waiter.fetch_add(1, Ordering::Relaxed),
        }
    }
}

pub struct CancellationFuture {
    token: CancellationToken,
    waiter_id: usize,
}

impl Future for CancellationFuture {
    type Output = CancellationReason;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(reason) = self.token.reason() {
            return Poll::Ready(reason);
        }
        let waker = context.waker().clone();
        let previous = self
            .token
            .state
            .waiters
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(self.waiter_id, waker);
        // Waker callbacks can reenter the token, including when releasing the old one.
        drop(previous);
        match self.token.reason() {
            Some(reason) => Poll::Ready(reason),
            None => Poll::Pending,
        }
    }
}

impl Drop for CancellationFuture {
    fn drop(&mut self) {
        let removed = self
            .token
            .state
            .waiters
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.waiter_id);
        drop(removed);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutomationIdKind {
    HarnessSession,
    HarnessTurn,
    AgentRun,
    WorkflowRun,
    ToolInvocation,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::Wake;

    #[test]
    fn cancellation_releases_replaced_and_removed_wakers_outside_waiter_lock() {
        struct ReentrantDrop {
            state: std::sync::Weak<CancellationState>,
            lock_free: Arc<AtomicU8>,
        }
        impl Wake for ReentrantDrop {
            fn wake(self: Arc<Self>) {}
        }
        impl Drop for ReentrantDrop {
            fn drop(&mut self) {
                let state = self.state.upgrade().unwrap();
                let lock_free = state.waiters.try_lock().is_ok();
                self.lock_free
                    .store(if lock_free { 2 } else { 1 }, Ordering::Relaxed);
                // Record a locked callback without reentering, so a regression cannot hang.
                if lock_free {
                    CancellationToken { state }.cancel(CancellationReason::User);
                }
            }
        }
        #[derive(Default)]
        struct Counter(AtomicUsize);
        impl Wake for Counter {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }

        for replace in [true, false] {
            let token = CancellationToken::default();
            let lock_free = Arc::new(AtomicU8::new(0));
            let mut future = Box::pin(token.cancelled());
            {
                let waker = Waker::from(Arc::new(ReentrantDrop {
                    state: Arc::downgrade(&token.state),
                    lock_free: lock_free.clone(),
                }));
                assert!(
                    future
                        .as_mut()
                        .poll(&mut Context::from_waker(&waker))
                        .is_pending()
                );
            }
            let replacement = Arc::new(Counter::default());
            let outcome = if replace {
                let waker = Waker::from(replacement.clone());
                Some(future.as_mut().poll(&mut Context::from_waker(&waker)))
            } else {
                None
            };
            drop(future);

            assert_eq!(lock_free.load(Ordering::Relaxed), 2, "replace={replace}");
            assert_eq!(token.reason(), Some(CancellationReason::User));
            assert!(!token.cancel(CancellationReason::DeadlineElapsed));
            assert!(token.state.waiters.lock().unwrap().is_empty());
            if let Some(outcome) = outcome {
                assert_eq!(outcome, Poll::Ready(CancellationReason::User));
                assert_eq!(replacement.0.load(Ordering::Relaxed), 1);
            }
        }
    }

    #[test]
    fn cancellation_wakes_every_waiter_and_unregisters_dropped_futures() {
        #[derive(Default)]
        struct Counter(AtomicUsize);
        impl Wake for Counter {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let token = CancellationToken::default();
        let counters = [
            Arc::new(Counter::default()),
            Arc::new(Counter::default()),
            Arc::new(Counter::default()),
        ];
        let mut futures = Vec::new();
        for counter in &counters {
            let waker = Waker::from(counter.clone());
            let mut future = Box::pin(token.cancelled());
            assert!(
                future
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
            futures.push(future);
        }
        drop(futures.pop());
        assert!(token.cancel(CancellationReason::User));
        assert_eq!(
            counters.map(|counter| counter.0.load(Ordering::Relaxed)),
            [1, 1, 0]
        );
        assert!(token.state.waiters.lock().unwrap().is_empty());
    }
}
