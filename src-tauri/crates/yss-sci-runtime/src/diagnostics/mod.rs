//! Model diagnostic entry points and report records.
pub mod serial_correlation;

pub mod residual;
pub use yss_sci::diagnostics::{comparison, design, influence, reclassification};
