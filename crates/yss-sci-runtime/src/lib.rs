//! Stateless scientific entry points grouped by domain for Node Kernel and focused SCI benchmarks.
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
pub mod inference;
pub mod longitudinal;
pub mod meta;
pub mod multivariate;
pub mod panel;
pub mod regression;
pub mod spatial;
pub mod survival;
pub mod time_series;
pub mod visualization;

mod error;

pub mod decision;
pub mod doe;
pub mod psychometrics;
pub mod quality;
pub mod report_display;

pub mod path;
pub mod power;
pub mod survey;
