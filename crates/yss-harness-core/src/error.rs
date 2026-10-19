use crate::KnowledgeError;
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AutomationIdentityError, CapabilityFailure,
    PersistenceFailure,
};

#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("automation identity is invalid")]
    Identity(#[from] AutomationIdentityError),
    #[error("harness persistence failed: {0}")]
    Persistence(#[from] PersistenceFailure),
    #[error("harness knowledge retrieval failed")]
    Knowledge(#[from] KnowledgeError),
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
