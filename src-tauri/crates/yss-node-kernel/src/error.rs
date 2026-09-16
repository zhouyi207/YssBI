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
    #[error("division by zero")]
    DivisionByZero,
    #[error("numeric result is not finite")]
    NonFiniteResult,
    #[error("execution kernel is not registered")]
    KernelNotFound,
}
