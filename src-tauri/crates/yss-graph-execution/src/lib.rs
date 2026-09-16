//! Graph execution plans, kernels, runtime state, and result lifecycle.

#![deny(unused_must_use)]

pub mod error;
pub mod finalization;
pub mod graph_preparation;
pub mod identity;
mod kernel_invocation;
pub mod package_preparation;
pub mod plan;
pub mod ports;
pub mod resource_preparation;
pub mod result;
pub mod result_store;
pub mod run_registry;
pub mod state;
