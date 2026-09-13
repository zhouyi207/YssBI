use yss_sci_contract::{SciError, SciOperationCode};

pub(crate) fn computation_failed(operation: SciOperationCode) -> SciError {
    SciError::ComputationFailed { operation }
}
