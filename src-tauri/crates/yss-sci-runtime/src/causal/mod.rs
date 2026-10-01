//! Econometric and causal analysis entry points.
pub mod did;
pub mod iv;
pub use yss_sci::causal::{designs, econometrics, treatment};
