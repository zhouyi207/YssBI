//! Stateless synchronous scientific-computing runtime for Execution and IPC Command.
//!
//! Execution and IPC Command call stateless computation functions directly.
//! Capability APIs prepare numeric inputs and project fitted results. The crate
//! composes the Rust algorithms with `yss_sci_contract` and does
//! not own Julia processes, project data, editing history, DuckDB state, DataFrame
//! export, Tauri transport, or UI state.

mod computation;
pub use computation::{acf_pacf, ols};

pub mod density;
pub mod hypothesis;
pub mod panel;
pub mod regression;
pub mod time_series;

mod error;

pub mod data;
