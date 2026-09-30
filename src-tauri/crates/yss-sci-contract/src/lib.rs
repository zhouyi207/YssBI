//! Backend-neutral contracts grouped by scientific domain.

pub mod causal;
pub mod density;
pub mod descriptive;
pub mod diagnostics;
pub mod distribution;
pub mod execution;
pub mod hypothesis;
pub mod panel;
pub mod regression;
pub mod time_series;

mod error;
mod observation;

pub use error::{SciError, SciInputViolation, SciOperationCode};
pub use observation::{CategoricalRole, MissingValuePolicy, StatisticalObservationMetadata};
