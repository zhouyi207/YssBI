use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FilesystemError {
    #[error("invalid filesystem root '{}': {message}", path.display())]
    InvalidRoot { path: PathBuf, message: String },
    #[error("filesystem root admission is closed: {message}")]
    RootAdmissionClosed { message: String },
    #[error("filesystem recovery is required: {message}")]
    RecoveryRequired { message: String },
    #[error("failed to prepare filesystem transaction: {message}")]
    TransactionPrepareFailed { message: String },
    #[error("failed to commit filesystem transaction: {message}")]
    TransactionCommitFailed { message: String },
    #[error("failed to roll back filesystem transaction: {message}")]
    TransactionRollbackFailed {
        message: String,
        recovery_required: bool,
    },
}

impl FilesystemError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidRoot { .. } => "invalid_filesystem_root",
            Self::RootAdmissionClosed { .. } => "filesystem_root_admission_closed",
            Self::RecoveryRequired { .. } => "filesystem_recovery_required",
            Self::TransactionPrepareFailed { .. } => "transaction_prepare_failed",
            Self::TransactionCommitFailed { .. } => "transaction_commit_failed",
            Self::TransactionRollbackFailed { .. } => "transaction_rollback_failed",
        }
    }

    pub const fn recovery_required(&self) -> bool {
        matches!(
            self,
            Self::RecoveryRequired { .. }
                | Self::TransactionRollbackFailed {
                    recovery_required: true,
                    ..
                }
        )
    }
}
