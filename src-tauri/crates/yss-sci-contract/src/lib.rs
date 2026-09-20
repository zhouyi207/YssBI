//! Backend-neutral scientific-computing contracts.

mod computation;
pub mod distribution;
mod error;
pub mod regression;
pub mod scientific;

pub use computation::{CategoricalRole, MissingValuePolicy, StatisticalObservationMetadata};
pub use error::{SciError, SciInputViolation, SciOperationCode};

pub mod hypothesis;

pub mod density;
pub mod panel;

pub mod serial_tests;
