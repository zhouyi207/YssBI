use crate::{ApprovalError, KnowledgeError, WorkflowCompileError, WorkflowRuntimeError};
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AutomationIdentityError, CapabilityFailure,
    IdGenerationFailure, PersistenceFailure,
};

#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("automation identity is invalid")]
    Identity(#[from] AutomationIdentityError),
    #[error("automation id generation failed")]
    IdGeneration(#[from] IdGenerationFailure),
    #[error("harness persistence failed: {0}")]
    Persistence(#[from] PersistenceFailure),
    #[error("harness knowledge retrieval failed")]
    Knowledge(#[from] KnowledgeError),
    #[error("harness approval operation failed")]
    Approval(#[from] ApprovalError),
    #[error("harness capability execution failed")]
    Capability(#[from] CapabilityFailure),
    #[error("harness session was not found")]
    SessionNotFound,
    #[error("harness session is not active")]
    SessionNotActive,
    #[error("harness session already has an active turn")]
    ConcurrentTurn,
    #[error("harness user message is invalid")]
    InvalidMessage,
    #[error("agent turn failed")]
    Agent(AgentDriverFailureCode),
    #[error("harness turn was cancelled")]
    Cancelled,
    #[error("workflow already has an active execution owner")]
    ConcurrentWorkflow,
    #[error("workflow compilation failed")]
    WorkflowCompile(#[from] WorkflowCompileError),
    #[error("workflow state transition failed")]
    WorkflowRuntime(#[from] WorkflowRuntimeError),
    #[error("workflow run was not found")]
    WorkflowNotFound,
    #[error("workflow immutable definition was not found")]
    WorkflowDefinitionNotFound,
    #[error("workflow originating turn was not found")]
    WorkflowTurnNotFound,
    #[error("workflow originating turn belongs to another session")]
    WorkflowTurnMismatch,
    #[error("workflow project binding changed")]
    WorkflowProjectChanged,
    #[error("workflow is paused")]
    WorkflowWaiting,
}

impl From<AgentDriverFailure> for HarnessError {
    fn from(error: AgentDriverFailure) -> Self {
        if error.code == AgentDriverFailureCode::Cancelled {
            Self::Cancelled
        } else {
            Self::Agent(error.code)
        }
    }
}
