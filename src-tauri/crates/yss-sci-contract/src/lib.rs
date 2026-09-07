//! Backend-neutral scientific-computing contracts.

mod computation;
mod control;
mod error;
pub mod regression;
pub mod scientific;

pub use computation::{
    CategoricalRole, MissingValuePolicy, StatisticalInput, StatisticalInputValidationError,
    StatisticalObservationMetadata, StatisticalScalar,
};
pub use control::{
    AbsoluteDeadline, CancelDeliveryControl, ExecutionControl, SciCancellationSource,
    SciCancellationToken,
};
pub use error::{SciError, SciInputViolation, SciOperationCode};

pub mod hypothesis;
