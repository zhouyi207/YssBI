use crate::error::CommandError;
use crate::event::{Event, EventProject, emit_project_event_result};
use tauri::{AppHandle, State};
use yss_application::execution::SessionCaptureError;
use yss_computation_settings::{
    ComputationSettingsMutationReceipt, ComputationSettingsMutationRequest,
    ComputationSettingsSnapshot,
};
use yss_project_identity::ProjectInstanceId;

#[tauri::command]
pub fn get_project_computation_settings(
    application: State<yss_application::execution::ApplicationState>,
    project_instance_id: String,
) -> Result<ComputationSettingsSnapshot, CommandError> {
    let expected = ProjectInstanceId::from_existing(project_instance_id);
    application
        .query_computation_settings(expected)
        .map_err(map_computation_settings_error)
}

#[tauri::command]
pub fn update_project_computation_settings(
    app: AppHandle,
    application: State<yss_application::execution::ApplicationState>,
    request: ComputationSettingsMutationRequest,
) -> Result<ComputationSettingsMutationReceipt, CommandError> {
    let result = application
        .update_computation_settings(request)
        .map_err(map_computation_settings_error)?;
    emit_project_event_result(
        &app,
        &Event::Project(EventProject::ComputationSettingsChanged {
            result: result.clone(),
        }),
    )
    .map_err(|error| CommandError::diagnosed("project_event_emit_failed", error))?;
    Ok(result)
}

fn map_computation_settings_error(
    error: yss_application::computation_settings::ComputationSettingsApplicationError,
) -> CommandError {
    use yss_application::computation_settings::ComputationSettingsApplicationError;
    match error {
        ComputationSettingsApplicationError::SessionCapture(error) => match error {
            SessionCaptureError::Inactive => CommandError::expected("stale_project_lifecycle"),
            SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            SessionCaptureError::Recovering => CommandError::expected("project_recovery_required")
                .with_details(serde_json::json!({ "recoveryRequired": true })),
        },
        ComputationSettingsApplicationError::ProjectIdentityMismatch { .. } => {
            CommandError::expected("stale_project_lifecycle")
        }
        ComputationSettingsApplicationError::Project(error) => {
            crate::commands::project_failure::application_project_command_error(error)
        }
        ComputationSettingsApplicationError::Validation(error) => {
            CommandError::diagnosed("invalid_computation_settings", error)
        }
        ComputationSettingsApplicationError::SessionChanged(error) => {
            CommandError::diagnosed("computation_settings_session_changed", error)
        }
        ComputationSettingsApplicationError::SessionRefresh(error) => {
            CommandError::diagnosed("computation_settings_session_refresh_failed", error)
        }
    }
}
