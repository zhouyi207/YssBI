use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};

pub(crate) fn invalid_input(operation: SciOperationCode, violation: SciInputViolation) -> SciError {
    SciError::InvalidInput {
        operation,
        violation,
    }
}

pub(crate) fn computation_failed(operation: SciOperationCode) -> SciError {
    SciError::ComputationFailed { operation }
}
