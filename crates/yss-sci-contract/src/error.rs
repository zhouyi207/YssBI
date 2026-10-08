//! Scientific-computing error model.

use crate::execution::{ScientificComputationError, ScientificInputViolation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SciOperationCode {
    Regression,
    InstrumentalVariables,
    Panel,
    Adf,
    VarFit,
    VarLagOrder,
    VecFit,
    VecRank,
    KernelDensity,
    AcfPacf,
    SerialTests,
    TTest,
    WaldTest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SciError {
    #[error("scientific input is invalid")]
    InvalidInput {
        operation: SciOperationCode,
        violation: ScientificInputViolation,
    },
    #[error("scientific computation failed")]
    ComputationFailed { operation: SciOperationCode },
}

impl SciError {
    /// Retain failure facts when the caller already identifies the operation.
    pub fn into_computation_error(self) -> ScientificComputationError {
        match self {
            Self::InvalidInput { violation, .. } => {
                ScientificComputationError::InvalidInput { violation }
            }
            Self::ComputationFailed { .. } => ScientificComputationError::ComputationFailed,
        }
    }
}
