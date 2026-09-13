#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RunPhase {
    Admission,
    PlanValidation,
    ResourcePreparation,
    Execution,
    Finalization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunFailureCode {
    KernelFailed,
    KernelNotFound,
    InvalidNumericInput,
    DivisionByZero,
    NonFiniteResult,
    DeadlineExceeded,
    ResourceUnavailable,
    FinalizationFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunFailure {
    pub code: RunFailureCode,
    pub phase: RunPhase,
    pub source: Option<crate::plan::PlanSourceIdentity>,
}
