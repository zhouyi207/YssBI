use crate::project::ProjectError;
use serde::Serialize;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Serialize, Clone)]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
    pub fn internal(error: impl fmt::Display) -> Self {
        Self::new("internal_error", error.to_string())
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::internal(message)
    }
}

impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.message
    }
}

impl From<ProjectError> for AppError {
    fn from(error: ProjectError) -> Self {
        let code = match error {
            ProjectError::FileNotFound(_) => "project_not_found",
            ProjectError::InvalidProjectFormat(_) => "invalid_project_format",
            ProjectError::Serialize(_) => "project_serialize_failed",
            ProjectError::Deserialize(_) => "project_deserialize_failed",
            ProjectError::Io(_) => "project_io_failed",
        };
        Self::new(code, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::AppError;

    #[test]
    fn serializes_stable_ipc_error_shape() {
        let value = serde_json::to_value(AppError::new("project_not_found", "missing")).unwrap();
        assert_eq!(value["code"], "project_not_found");
        assert_eq!(value["message"], "missing");
        assert!(value.get("details").is_none());
    }
}
