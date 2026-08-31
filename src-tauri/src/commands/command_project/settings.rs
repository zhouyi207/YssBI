use crate::application::execution::SessionCaptureError;
use crate::error::CommandError;
#[cfg(test)]
use crate::event::emit_project_event;
use crate::event::{Event, EventProject, emit_project_event_result};
use tauri::{AppHandle, State};
use yss_computation_settings::{
    ComputationSettingsMutationReceipt, ComputationSettingsMutationRequest,
    ComputationSettingsSnapshot,
};
#[cfg(test)]
use yss_project::ProjectState;
use yss_project_identity::ProjectInstanceId;

#[tauri::command]
pub fn get_project_computation_settings(
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: String,
) -> Result<ComputationSettingsSnapshot, CommandError> {
    let expected = ProjectInstanceId::from_existing(project_instance_id);
    application
        .query_computation_settings(expected)
        .map_err(map_computation_settings_error)
}

#[cfg(test)]
pub(crate) fn update_project_computation_settings_with_emitter(
    state: &ProjectState,
    request: ComputationSettingsMutationRequest,
    mut emit: impl FnMut(Event),
) -> Result<ComputationSettingsMutationReceipt, CommandError> {
    let result = state
        .update_computation_settings_transaction(request)
        .map_err(crate::commands::project_failure::application_project_command_error)?;
    emit(Event::Project(EventProject::ComputationSettingsChanged {
        result: result.clone(),
    }));
    Ok(result)
}

#[tauri::command]
pub fn update_project_computation_settings(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
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
    error: crate::application::computation_settings::ComputationSettingsApplicationError,
) -> CommandError {
    use crate::application::computation_settings::ComputationSettingsApplicationError;
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
