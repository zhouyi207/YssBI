//! Provider-neutral statistical automation session, tool, and workflow authority.

#![forbid(unsafe_code)]

mod agents;
mod approval;
mod conversation;
mod error;
mod events;
mod host;
mod knowledge;
mod orchestration;
mod planner;
mod ports;
mod skills;
mod tools;
mod workflow;

pub use agents::{
    AGENT_DEFINITIONS, AgentDefinition, agent_definition, authorize_agent_capability,
    authorize_agent_resource,
};
pub use approval::{ApprovalError, ApprovalService};
pub use error::HarnessError;
pub use host::{HarnessHost, HarnessSessionAccess};
pub use knowledge::{
    KnowledgeError, KnowledgeQuery, KnowledgeService, install_builtin_statistical_knowledge,
};
pub use planner::{MethodRegistry, StatisticalPlanner, StatisticalPlannerError};
pub use ports::HarnessPorts;
pub use skills::{SkillError, SkillRegistry};
pub use tools::ToolRegistry;
pub use workflow::{CompiledWorkflow, WorkflowCompileError, WorkflowRuntime, WorkflowRuntimeError};

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
