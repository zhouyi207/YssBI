use tauri::State;
use yss_settings::{
    SettingsMutationReceipt, SettingsMutationRequest, SettingsSnapshot, SettingsStore,
    SettingsStoreError,
};

use crate::error::CommandError;

#[tauri::command]
pub fn get_application_settings(settings: State<'_, SettingsStore>) -> SettingsSnapshot {
    settings.snapshot()
}

#[tauri::command]
pub fn update_application_settings(
    settings: State<'_, SettingsStore>,
    request: SettingsMutationRequest,
) -> Result<SettingsMutationReceipt, CommandError> {
    settings.update(request).map_err(map_settings_error)
}

fn map_settings_error(error: SettingsStoreError) -> CommandError {
    match error {
        SettingsStoreError::RevisionConflict { .. } => {
            CommandError::expected("settings_revision_conflict")
        }
        SettingsStoreError::Validation(error) => {
            CommandError::diagnosed("invalid_application_settings", error)
        }
        error => CommandError::diagnosed("settings_store_unavailable", error),
    }
}
