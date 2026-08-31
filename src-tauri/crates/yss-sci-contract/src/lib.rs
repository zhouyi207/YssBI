//! Backend-neutral scientific-computing contracts.

mod computation;
mod control;
mod error;

pub use computation::{
    CategoricalRole, MissingValuePolicy, NumericTolerance, SciComputationSettings,
    StatisticalInput, StatisticalInputValidationError, StatisticalObservationMetadata,
    StatisticalScalar, StatisticalSettingSource,
};
pub use control::{
    AbsoluteDeadline, CancelDeliveryControl, ExecutionControl, SciCancellationSource,
    SciCancellationToken,
};
pub use error::{SciError, SciInputViolation, SciOperationCode};
