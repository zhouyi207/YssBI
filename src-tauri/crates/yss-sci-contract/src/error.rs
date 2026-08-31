//! Scientific-computing error model.

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SciInputViolation {
    EmptyInput,
    NonFiniteInput,
    ShapeMismatch,
    ParameterOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SciError {
    #[error("scientific input is invalid")]
    InvalidInput {
        operation: SciOperationCode,
        violation: SciInputViolation,
    },
    #[error("scientific computation failed")]
    ComputationFailed { operation: SciOperationCode },
}

impl SciError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "sci_invalid_input",
            Self::ComputationFailed { .. } => "sci_computation_failed",
        }
    }
}
