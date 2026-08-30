use std::path::PathBuf;
use thiserror::Error;
use yss_resource_naming::ResourceNameValidationError;
use yss_worksheet_document::{WorksheetResourcePath, WorksheetResourcePathError};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectFilesystemError {
    #[error("invalid resource name: {0}")]
    InvalidResourceName(#[from] ResourceNameValidationError),
    #[error("invalid worksheet resource path: {0}")]
    InvalidWorksheetResourcePath(#[from] WorksheetResourcePathError),
    #[error("invalid project root '{}': {message}", path.display())]
    InvalidRoot { path: PathBuf, message: String },
    #[error("graph resource '{path}' is structurally invalid")]
    InvalidGraphDocument {
        path: super::GraphResourcePath,
        message: String,
    },
    #[error("stale project lifecycle: {message}")]
    StaleProjectLifecycle { message: String },
    #[error("stale resource lifecycle: {message}")]
    StaleResourceLifecycle { message: String },
    #[error("catalog resource stale: {message}")]
    CatalogResourceStale { message: String },

    #[error("result source read failed: {message}")]
    ResultSourceReadFailed { message: String },
    #[error("resource revision overflow for '{resource}' at {retained}")]
    ResourceRevisionOverflow { resource: String, retained: u64 },
    #[error("resource name conflict: {message}")]
    ResourceNameConflict { message: String },
    #[error("worksheet resource '{}' was not found", path.as_str())]
    WorksheetNotFound { path: WorksheetResourcePath },
    #[error("resource revision conflict: {message}")]
    ResourceRevisionConflict { message: String },
    #[error("duplicate project operation: {message}")]
    DuplicateOperation { message: String },
    #[error("project filesystem transaction is busy: {message}")]
    FilesystemTransactionBusy { message: String },
    #[error("project lifecycle admission is closed: {message}")]
    ProjectLifecycleAdmissionClosed { message: String },
    #[error("project activation generation is exhausted")]
    ActivationGenerationExhausted,
    #[error("project authority generation is exhausted")]
    AuthorityGenerationExhausted,
    #[error("resource publication revision is exhausted")]
    PublicationRevisionExhausted,
    #[error("computation settings revision is exhausted")]
    ComputationSettingsRevisionExhausted,
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
            Self::InvalidResourceName(source) => resource_name_error_code(source),
            Self::InvalidWorksheetResourcePath(source) => match source {
                WorksheetResourcePathError::InvalidName(source) => resource_name_error_code(source),
                _ => "invalid_resource_name",
            },
            Self::InvalidRoot { .. } => "invalid_project_root",
            Self::InvalidGraphDocument { .. } => "invalid_graph_document",
            Self::StaleProjectLifecycle { .. } => "stale_project_lifecycle",
            Self::StaleResourceLifecycle { .. } => "stale_resource_lifecycle",
            Self::CatalogResourceStale { .. } => "catalog_resource_stale",

            Self::ResultSourceReadFailed { .. } => "result_source_read_failed",
            Self::ResourceRevisionOverflow { .. } => "resource_revision_overflow",
            Self::ResourceNameConflict { .. } => "resource_name_conflict",
            Self::WorksheetNotFound { .. } => "resource_not_found",
            Self::ResourceRevisionConflict { .. } => "resource_revision_conflict",
            Self::DuplicateOperation { .. } => "duplicate_operation",
            Self::FilesystemTransactionBusy { .. } => "filesystem_transaction_busy",
            Self::ProjectLifecycleAdmissionClosed { .. } => "project_lifecycle_admission_closed",
            Self::ActivationGenerationExhausted => "project_activation_generation_exhausted",
            Self::AuthorityGenerationExhausted => "project_authority_generation_exhausted",
            Self::PublicationRevisionExhausted => "publication_revision_exhausted",
            Self::ComputationSettingsRevisionExhausted => "computation_settings_revision_exhausted",
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

const fn resource_name_error_code(error: &ResourceNameValidationError) -> &'static str {
    match error {
        ResourceNameValidationError::NotNfc => "resource_name_not_normalized",
        ResourceNameValidationError::Reserved => "resource_name_reserved",
        ResourceNameValidationError::TooLong => "resource_name_too_long",
        ResourceNameValidationError::Empty
        | ResourceNameValidationError::ForbiddenCharacter(_)
        | ResourceNameValidationError::InvalidSpacing => "invalid_resource_name",
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
    InvalidGraphDocument { path: PathBuf, message: String },
}

impl From<yss_graph_document::GraphResourcePathError> for ProjectError {
    fn from(source: yss_graph_document::GraphResourcePathError) -> Self {
        Self::InvalidProjectFormat(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::ProjectFilesystemError;
    use yss_resource_naming::ResourceNameValidationError;
    use yss_worksheet_document::WorksheetResourcePathError;

    #[test]
    fn resource_name_errors_have_stable_ipc_codes() {
        for (source, code) in [
            (ResourceNameValidationError::Empty, "invalid_resource_name"),
            (
                ResourceNameValidationError::ForbiddenCharacter('?'),
                "invalid_resource_name",
            ),
            (
                ResourceNameValidationError::InvalidSpacing,
                "invalid_resource_name",
            ),
            (
                ResourceNameValidationError::NotNfc,
                "resource_name_not_normalized",
            ),
            (
                ResourceNameValidationError::Reserved,
                "resource_name_reserved",
            ),
            (
                ResourceNameValidationError::TooLong,
                "resource_name_too_long",
            ),
        ] {
            assert_eq!(
                ProjectFilesystemError::InvalidResourceName(source).code(),
                code
            );
        }
    }

    #[test]
    fn resource_path_errors_preserve_name_error_ipc_codes() {
        let not_normalized = ProjectFilesystemError::InvalidWorksheetResourcePath(
            WorksheetResourcePathError::InvalidName(ResourceNameValidationError::NotNfc),
        );
        let structurally_invalid = ProjectFilesystemError::InvalidWorksheetResourcePath(
            WorksheetResourcePathError::WrongDirectory,
        );

        assert_eq!(not_normalized.code(), "resource_name_not_normalized");
        assert_eq!(structurally_invalid.code(), "invalid_resource_name");
    }
}
