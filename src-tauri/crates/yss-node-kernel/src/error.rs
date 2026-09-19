use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("kernel execution was cancelled")]
    Cancelled,
    #[error("kernel execution deadline was exceeded")]
    DeadlineExceeded,
    #[error("kernel execution failed")]
    Failed,
    #[error("numeric input has an incompatible runtime type")]
    InvalidNumericInput,
    #[error("input shapes do not agree")]
    ShapeMismatch,
    #[error("kernel parameters are invalid")]
    InvalidParameter,
    #[error("input series are not aligned")]
    UnalignedSeries,
    #[error("kernel memory budget was exceeded")]
    BudgetExceeded,
    #[error("kernel input layout does not match its contract")]
    InputLayoutMismatch,
    #[error("kernel output does not match its contract")]
    OutputContractMismatch,
    #[error("scientific computation failed")]
    ScientificFailure,
    #[error("division by zero")]
    DivisionByZero,
    #[error("numeric result is not finite")]
    NonFiniteResult,
    #[error("execution kernel is not registered")]
    KernelNotFound,
}
