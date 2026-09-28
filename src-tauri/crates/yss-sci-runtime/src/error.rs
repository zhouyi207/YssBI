use yss_sci_contract::{SciError, SciOperationCode};

pub(crate) fn computation_failed(operation: SciOperationCode) -> SciError {
    SciError::ComputationFailed { operation }
}

use yss_sci_contract::SciInputViolation;
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
            invalid(match violation {
                SciInputViolation::EmptyInput => ScientificInputViolation::EmptyInput,
                SciInputViolation::NonFiniteInput => ScientificInputViolation::NonFiniteInput,
                SciInputViolation::ShapeMismatch => ScientificInputViolation::ShapeMismatch,
                SciInputViolation::ParameterOutOfRange => {
                    ScientificInputViolation::ParameterOutOfRange
                }
            })
        }
        SciError::ComputationFailed { .. } => ScientificComputationError::ComputationFailed,
    }
}
