//! Reusable end-to-end fixtures and assertions for the node system.
//!
//! These helpers intentionally build only the current protocol/registry/document
//! pipeline. Stable node and port identities are accepted at the public graph
//! boundary; document UUIDs and plan-local handles remain fixture details.

mod assertions;
mod builders;
#[cfg(test)]
pub(crate) mod contracts;
mod determinism;
mod protocol;
mod runtime;
mod snapshots;
pub(crate) mod source_audit;

pub use assertions::{CompileAssertions, RunAssertions, compile_assertions, run_assertions};
pub use builders::{
    EmptyResourceSnapshot, TestGraphBuilder, TestNode, TestProvider, TestProviderBuilder,
};
pub use determinism::{assert_locale_invariance, assert_random_insertion_order_determinism};
pub(crate) use protocol::TestProtocolBuilder;
pub use runtime::{
    KernelRecord, KernelRecorder, NoFunctionPlans, ResourceLeakTracker, tracked_requirement,
};
pub use snapshots::{canonical_analysis, canonical_document, canonical_json, plan_debug_snapshot};

#[cfg(test)]
mod tests;
