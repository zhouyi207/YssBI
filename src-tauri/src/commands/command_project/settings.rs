use crate::error::CommandError;
use crate::event::{Event, EventProject, emit_project_event};
use crate::project::{
    ComputationSettingsMutationReceipt, ComputationSettingsMutationRequest,
    ComputationSettingsSnapshot, ProjectInstanceId, ProjectState,
};
use tauri::{AppHandle, State};

#[tauri::command]
pub fn get_project_computation_settings(
    state: State<ProjectState>,
    project_instance_id: String,
) -> Result<ComputationSettingsSnapshot, CommandError> {
    let expected = ProjectInstanceId::from_existing(project_instance_id);
    let result = state
        .get_computation_settings()
        .map_err(CommandError::from)?;
    if result.project_instance_id != expected {
        return Err(CommandError::expected("stale_project_lifecycle"));
    }
    Ok(result)
}

pub(crate) fn update_project_computation_settings_with_emitter(
    state: &ProjectState,
    request: ComputationSettingsMutationRequest,
    mut emit: impl FnMut(Event),
) -> Result<ComputationSettingsMutationReceipt, CommandError> {
    let result = state
        .update_computation_settings_transaction(request)
        .map_err(CommandError::from)?;
    emit(Event::Project(EventProject::ComputationSettingsChanged {
        result: result.clone(),
    }));
    Ok(result)
}

#[tauri::command]
pub fn update_project_computation_settings(
    app: AppHandle,
    state: State<ProjectState>,
    request: ComputationSettingsMutationRequest,
) -> Result<ComputationSettingsMutationReceipt, CommandError> {
    update_project_computation_settings_with_emitter(state.inner(), request, |event| {
        emit_project_event(&app, event)
    })
}
