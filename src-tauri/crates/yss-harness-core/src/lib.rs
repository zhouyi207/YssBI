//! Provider-neutral statistical automation session, tool, and workflow authority.

#![forbid(unsafe_code)]

mod approval;
mod host;
mod knowledge;
mod memory;
mod planner;
mod skills;
mod tools;
mod workflow;

pub use approval::{ApprovalError, ApprovalService};
pub use host::{HarnessError, HarnessHost, HarnessPorts};
pub use knowledge::{
    KnowledgeError, KnowledgeQuery, KnowledgeService, install_builtin_statistical_knowledge,
};
pub use memory::{MemoryError, MemoryService};
pub use planner::{MethodRegistry, StatisticalPlanner, StatisticalPlannerError};
pub use skills::{SkillError, SkillRegistry};
pub use tools::ToolRegistry;
pub use workflow::{
    CompiledWorkflow, WorkflowCompileError, WorkflowRuntime, WorkflowRuntimeError,
    dataset_quality_review_workflow,
};

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
