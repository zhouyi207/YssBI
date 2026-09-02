//! Authoritative execution plans, ports, runtime state, and result lifecycle.

#![deny(unused_must_use)]

pub mod canonical;
pub mod error;
pub mod finalization;
pub mod identity;
pub mod package_preparation;
pub mod plan;
pub mod ports;
pub mod resource_preparation;
pub mod result;
pub mod result_store;
pub mod run_output;
pub mod run_registry;
pub mod settings;
pub mod state;
pub mod value;
