//! Domain-neutral Harness engine: sessions, turns, Manager–Worker orchestration, the durable
//! tool ledger, knowledge retrieval, workflows and approvals. Domain tools, roles and authority
//! are supplied by a `HarnessExtension`.

#![forbid(unsafe_code)]

mod approval;
mod conversation;
mod error;
mod events;
mod host;
mod knowledge;
mod orchestration;
mod ports;
mod tools;
mod workflow;

pub use approval::{ApprovalError, ApprovalService};
pub use error::HarnessError;
pub use host::{HarnessHost, HarnessSessionAccess};
pub use knowledge::{KnowledgeError, KnowledgeQuery, KnowledgeService};
pub use ports::HarnessPorts;
pub use workflow::{CompiledWorkflow, WorkflowCompileError, WorkflowRuntime, WorkflowRuntimeError};

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
