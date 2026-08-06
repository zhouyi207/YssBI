use crate::node_system::document::DocumentError;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectFilesystemError {
    #[error("built-in node system initialization failed: {0}")]
    BuiltinInitialization(#[from] crate::node_system::catalog::BuiltinInitializationError),
    #[error("invalid project root '{}': {message}", path.display())]
    InvalidRoot { path: PathBuf, message: String },
    #[error("graph resource '{path}' is structurally invalid")]
    InvalidGraphDocument {
        path: super::GraphResourcePath,
        #[source]
        source: DocumentError,
    },
    #[error("stale project lifecycle: {message}")]
    StaleProjectLifecycle { message: String },
    #[error("catalog resource stale: {message}")]
    CatalogResourceStale { message: String },
    #[error("resource revision conflict: {message}")]
    ResourceRevisionConflict { message: String },
    #[error("duplicate project operation: {message}")]
    DuplicateOperation { message: String },
    #[error("project filesystem transaction is busy: {message}")]
    FilesystemTransactionBusy { message: String },
    #[error("project lifecycle admission is closed: {message}")]
    ProjectLifecycleAdmissionClosed { message: String },
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
            Self::BuiltinInitialization(_) => "builtin_initialization_failed",
            Self::InvalidRoot { .. } => "invalid_project_root",
            Self::InvalidGraphDocument { .. } => "invalid_graph_document",
            Self::StaleProjectLifecycle { .. } => "stale_project_lifecycle",
            Self::CatalogResourceStale { .. } => "catalog_resource_stale",
            Self::ResourceRevisionConflict { .. } => "resource_revision_conflict",
            Self::DuplicateOperation { .. } => "duplicate_operation",
            Self::FilesystemTransactionBusy { .. } => "filesystem_transaction_busy",
            Self::ProjectLifecycleAdmissionClosed { .. } => "project_lifecycle_admission_closed",
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

    #[error("graph file '{}' is structurally invalid", path.display())]
    InvalidGraphDocument {
        path: PathBuf,
        #[source]
        source: DocumentError,
    },
}
