//! Application-facing synchronous scientific-computing runtime.
//!
//! The composition root injects this crate's scientific backend implementation.
//! Capability APIs prepare numeric inputs and project fitted results. The crate
//! composes the Rust algorithms with `yss_sci_contract` and does
//! not own Julia processes, project data, editing history, DuckDB state, DataFrame
//! export, Tauri transport, or UI state.

mod service;
pub use service::SciRuntimeBackend;

pub mod density;
pub mod hypothesis;
pub mod panel;
pub mod regression;
pub mod time_series;

mod error;

pub mod data;
