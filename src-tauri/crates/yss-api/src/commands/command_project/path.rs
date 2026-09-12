use crate::error::CommandError;
use yss_project_registry::{
    default_project_parent_directory as default_project_parent_directory_impl,
    validate_new_project_path as validate_new_project_path_impl,
};

#[tauri::command]
pub fn default_project_parent_directory() -> Result<String, CommandError> {
    default_project_parent_directory_impl().map_err(CommandError::internal)
}

#[tauri::command]
pub fn validate_new_project_path(path: String) -> Result<(), CommandError> {
    validate_new_project_path_impl(&path).map_err(|error| CommandError::expected(error.code()))
}
