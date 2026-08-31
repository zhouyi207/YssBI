//! Application-facing synchronous scientific-computing runtime.
//!
//! Application and backend adapters call this crate instead of depending directly
//! on `yss_sci`. It composes the Rust algorithms with `yss_sci_contract` and does
//! not own Julia processes, project data, editing history, DuckDB state, DataFrame
//! export, Tauri transport, or UI state.

pub mod api;
pub mod backends;
pub mod engine;
pub mod models;
