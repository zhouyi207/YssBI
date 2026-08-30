use crate::application::project_failure::ApplicationProjectFailure;
use crate::error::CommandError;

pub(crate) fn application_project_command_error(
    error: impl Into<ApplicationProjectFailure>,
) -> CommandError {
    let error = error.into();
    let recovery_required = error.recovery_required();
    let command_error = CommandError::expected(error.code());
    if recovery_required {
        command_error.with_details(serde_json::json!({ "recoveryRequired": true }))
    } else {
        command_error
    }
}

#[cfg(test)]
mod tests {
    use super::application_project_command_error;
    use yss_project_filesystem::ProjectFilesystemError;

    #[test]
    fn maps_expected_project_failure_without_diagnostic_incident() {
        let error =
            application_project_command_error(ProjectFilesystemError::StaleProjectLifecycle {
                message: "stale test session".into(),
            });

        assert_eq!(error.code(), "stale_project_lifecycle");
        assert!(error.details().is_none());
        assert!(error.incident_id().is_none());
    }

    #[test]
    fn retains_recovery_required_wire_detail() {
        let error =
            application_project_command_error(ProjectFilesystemError::ProjectRecoveryRequired {
                message: "test recovery".into(),
            });

        assert_eq!(error.code(), "project_recovery_required");
        assert_eq!(
            error
                .details()
                .and_then(|details| details.get("recoveryRequired")),
            Some(&serde_json::Value::Bool(true))
        );
        assert!(error.incident_id().is_none());
    }
}
