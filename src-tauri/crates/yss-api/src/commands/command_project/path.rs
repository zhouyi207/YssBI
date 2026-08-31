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

#[cfg(test)]
mod tests {
    use super::validate_new_project_path;

    #[test]
    fn path_validation_returns_stable_command_codes_without_backend_prose() {
        let empty = validate_new_project_path(String::new()).expect_err("empty path rejected");
        assert_eq!(empty.code(), "project_path_empty");
        let wire = serde_json::to_value(empty).expect("serialize path validation error");
        assert_eq!(wire["details"], serde_json::Value::Null);
        assert_eq!(wire["incidentId"], serde_json::Value::Null);
        assert!(wire.get("message").is_none());
    }
}
