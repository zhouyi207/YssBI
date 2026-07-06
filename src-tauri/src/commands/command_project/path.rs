use crate::project::{
    ProjectPathValidation, default_project_parent_directory as default_project_parent_directory_impl,
    validate_new_project_path as validate_new_project_path_impl,
};

#[tauri::command]
pub fn default_project_parent_directory() -> Result<String, String> {
    default_project_parent_directory_impl()
}

#[tauri::command]
pub fn validate_new_project_path(path: String) -> ProjectPathValidation {
    validate_new_project_path_impl(&path)
}
