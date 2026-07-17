//! Application scientific-computing boundary.
//!
//! Graph nodes and Tauri commands call this module instead of directly
//! depending on `yss_sci` or Julia worker internals. Backends may use the
//! legacy Rust `yss-sci` crate or the Julia worker. Validation belongs in tests
//! against golden results. This module does not own project data, editing history, DuckDB state,
//! DataFrame export, or UI state.

pub mod api;
pub mod backends;
pub mod engine;
pub mod error;
