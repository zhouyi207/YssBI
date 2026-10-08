use yss_sci_contract::{SciError, SciOperationCode};

pub(crate) fn computation_failed(operation: SciOperationCode) -> SciError {
    SciError::ComputationFailed { operation }
}

use yss_sci_contract::execution::{ScientificComputationError, ScientificInputViolation};

pub(crate) const fn invalid(violation: ScientificInputViolation) -> ScientificComputationError {
    ScientificComputationError::InvalidInput { violation }
}
