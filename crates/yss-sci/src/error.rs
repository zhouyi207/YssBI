use yss_sci_contract::{
    SciError, SciOperationCode,
    execution::{ScientificComputationError, ScientificInputViolation},
};

pub(crate) fn invalid_input(
    operation: SciOperationCode,
    violation: ScientificInputViolation,
) -> SciError {
    SciError::InvalidInput {
        operation,
        violation,
    }
}

pub(crate) fn computation_failed(operation: SciOperationCode) -> SciError {
    SciError::ComputationFailed { operation }
}

pub(crate) fn invalid(violation: ScientificInputViolation) -> ScientificComputationError {
    ScientificComputationError::InvalidInput { violation }
}

pub(crate) fn parameter() -> ScientificComputationError {
    invalid(ScientificInputViolation::ParameterOutOfRange)
}

pub(crate) fn failed() -> ScientificComputationError {
    ScientificComputationError::ComputationFailed
}

/// A computed value that is not finite means the numerical method failed, not the input.
pub(crate) fn finite(value: f64) -> Result<f64, ScientificComputationError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(failed())
    }
}
