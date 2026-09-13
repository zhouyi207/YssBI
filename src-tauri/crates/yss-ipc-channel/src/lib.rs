//! Desktop streaming adapters and their bounded delivery lifecycles.
pub mod diagnostics;
pub mod execution;
mod harness;
pub mod harness_graph;
pub mod project_progress;
pub use harness::HarnessChannelHub;
pub use harness_graph::HarnessGraphClientHub;
