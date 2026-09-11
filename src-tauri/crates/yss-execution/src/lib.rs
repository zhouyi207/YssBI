//! Authoritative execution plans, ports, runtime state, and result lifecycle.

#![deny(unused_must_use)]

pub mod error;
pub mod finalization;
pub mod identity;
mod numeric;
pub mod package_preparation;
pub mod plan;
pub mod ports;
mod relational;
pub mod resource_preparation;
pub mod result;
pub mod result_store;
pub mod run_output;
pub mod run_registry;
pub mod state;
mod statistics;
pub mod value;
