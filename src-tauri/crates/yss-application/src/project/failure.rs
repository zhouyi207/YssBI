use thiserror::Error;
use yss_project::ProjectOperationError;

/// Application-owned view of a Project failure that may cross into a delivery
/// adapter. Transport code can classify the failure without depending on the
/// Project layer's concrete error type.
#[derive(Debug, Clone, Error)]
#[error(transparent)]
pub struct ApplicationProjectFailure(#[from] ProjectOperationError);

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
    use yss_project::ProjectOperationError;

    #[test]
    fn preserves_recovery_classification() {
        let failure =
            ApplicationProjectFailure::from(ProjectOperationError::TransactionRollbackFailed {
                message: "rollback test failure".into(),
                recovery_required: true,
            });

        assert_eq!(failure.code(), "transaction_rollback_failed");
        assert!(failure.recovery_required());
    }
}
