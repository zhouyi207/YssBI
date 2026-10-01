//! Stateless scientific entry points grouped by domain for Node Kernel and IPC commands.
//!
//! Capability APIs prepare numeric inputs and project fitted results. The crate
//! composes the Rust algorithms with `yss_sci_contract` and does
//! not own Julia processes, project data, editing history, database state, DataFrame
//! export, Tauri transport, or UI state.
pub mod anova;
pub mod association;
pub mod causal;
pub mod descriptive;
pub mod diagnostics;
pub mod distribution;
pub mod hypothesis;
pub mod longitudinal;
pub mod multivariate;
pub mod panel;
pub mod regression;
pub mod spatial;
pub mod survival;
pub mod time_series;
pub mod visualization;

mod error;

pub mod report_display;
