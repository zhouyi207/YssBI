use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectFilesystemError {
    #[error("invalid project root '{}': {message}", path.display())]
    InvalidRoot { path: PathBuf, message: String },
    #[error("stale project lifecycle: {message}")]
    StaleProjectLifecycle { message: String },
    #[error("resource revision conflict: {message}")]
    ResourceRevisionConflict { message: String },
    #[error("project filesystem transaction is busy: {message}")]
    FilesystemTransactionBusy { message: String },
    #[error("project requires recovery before mutations can continue: {message}")]
    ProjectRecoveryRequired { message: String },
    #[error("failed to prepare project filesystem transaction: {message}")]
    TransactionPrepareFailed { message: String },
    #[error("failed to commit project filesystem transaction: {message}")]
    TransactionCommitFailed { message: String },
    #[error("failed to roll back project filesystem transaction: {message}")]
    TransactionRollbackFailed {
        message: String,
        recovery_required: bool,
    },
}

impl ProjectFilesystemError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidRoot { .. } => "invalid_project_root",
            Self::StaleProjectLifecycle { .. } => "stale_project_lifecycle",
            Self::ResourceRevisionConflict { .. } => "resource_revision_conflict",
            Self::FilesystemTransactionBusy { .. } => "filesystem_transaction_busy",
            Self::ProjectRecoveryRequired { .. } => "project_recovery_required",
            Self::TransactionPrepareFailed { .. } => "transaction_prepare_failed",
            Self::TransactionCommitFailed { .. } => "transaction_commit_failed",
            Self::TransactionRollbackFailed { .. } => "transaction_rollback_failed",
        }
    }

    pub const fn recovery_required(&self) -> bool {
        matches!(
            self,
            Self::ProjectRecoveryRequired { .. }
                | Self::TransactionRollbackFailed {
                    recovery_required: true,
                    ..
                }
        )
    }
}

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("failed to serialize project data")]
    Serialize(#[source] serde_json::Error),

    #[error("failed to deserialize project data")]
    Deserialize(#[source] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("project file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("invalid project format: {0}")]
    InvalidProjectFormat(String),
}
