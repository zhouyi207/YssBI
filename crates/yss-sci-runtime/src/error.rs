use yss_sci_contract::{SciError, SciOperationCode};

pub(crate) fn computation_failed(operation: SciOperationCode) -> SciError {
    SciError::ComputationFailed { operation }
}

use yss_sci_contract::execution::{ScientificComputationError, ScientificInputViolation};

pub(crate) const fn invalid(violation: ScientificInputViolation) -> ScientificComputationError {
    ScientificComputationError::InvalidInput { violation }
}

pub(crate) fn map_sci_error(error: SciError) -> ScientificComputationError {
    match error {
        SciError::InvalidInput {
            operation,
            violation,
        } => {
            if !matches!(
                operation,
                SciOperationCode::AcfPacf | SciOperationCode::Regression
            ) {
                return ScientificComputationError::ComputationFailed;
            }
            invalid(violation)
        }
        SciError::ComputationFailed { .. } => ScientificComputationError::ComputationFailed,
    }
}
