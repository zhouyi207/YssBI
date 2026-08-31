use thiserror::Error;
use yss_project_filesystem::ProjectFilesystemError;

/// Application-owned view of a Project failure that may cross into a delivery
/// adapter. Transport code can classify the failure without depending on the
/// Project layer's concrete error type.
#[derive(Debug, Clone, Error)]
#[error(transparent)]
pub struct ApplicationProjectFailure(#[from] ProjectFilesystemError);

impl ApplicationProjectFailure {
    pub const fn code(&self) -> &'static str {
        self.0.code()
    }

    pub const fn recovery_required(&self) -> bool {
        self.0.recovery_required()
    }
}

#[cfg(test)]
mod tests {
    use super::ApplicationProjectFailure;
    use yss_project_filesystem::ProjectFilesystemError;

    #[test]
    fn preserves_project_code_without_exposing_the_concrete_error_to_transport() {
        let failure =
            ApplicationProjectFailure::from(ProjectFilesystemError::StaleProjectLifecycle {
                message: "stale test session".into(),
            });

        assert_eq!(failure.code(), "stale_project_lifecycle");
        assert!(!failure.recovery_required());
        assert_eq!(
            failure.to_string(),
            "stale project lifecycle: stale test session"
        );
    }

    #[test]
    fn preserves_recovery_classification() {
        let failure =
            ApplicationProjectFailure::from(ProjectFilesystemError::TransactionRollbackFailed {
                message: "rollback test failure".into(),
                recovery_required: true,
            });

        assert_eq!(failure.code(), "transaction_rollback_failed");
        assert!(failure.recovery_required());
    }
}
