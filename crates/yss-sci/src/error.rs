use yss_sci_contract::{SciError, SciOperationCode, execution::ScientificInputViolation};

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
