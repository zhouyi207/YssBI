use thiserror::Error;
use yss_relational_contract::RelationError;

pub fn kernel_error(error: RelationError) -> KernelError {
    match error {
        RelationError::Cancelled => KernelError::Cancelled,
        RelationError::DeadlineExceeded => KernelError::DeadlineExceeded,
        RelationError::DivisionByZero => KernelError::DivisionByZero,
        RelationError::NonFiniteResult => KernelError::NonFiniteResult,
        RelationError::InvalidInput => KernelError::InvalidNumericInput,
        RelationError::UnalignedSeries => KernelError::UnalignedSeries,
        RelationError::ShapeMismatch => KernelError::ShapeMismatch,
        RelationError::GroupSchemaMismatch => KernelError::GroupSchemaMismatch,
        RelationError::GroupKeyCollision => KernelError::GroupKeyCollision,
        RelationError::MemoryLimitExceeded => KernelError::BudgetExceeded,
        RelationError::InvalidConversion => KernelError::InvalidParameter,
        _ => KernelError::Failed,
    }
}

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
    #[error("group functions returned inconsistent schemas")]
    GroupSchemaMismatch,
    #[error("group key output names collide with returned columns")]
    GroupKeyCollision,
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
