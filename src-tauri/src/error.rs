use crate::project::{ProjectError, ProjectFilesystemError};
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMutationErrorDetailsDto {
    pub category: &'static str,
}

impl GraphMutationErrorDetailsDto {
    pub const VALUE: Self = Self {
        category: "graphMutation",
    };
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let resource_context = self
            .details
            .as_ref()
            .and_then(Value::as_object)
            .filter(|details| {
                details.get("resourceKind").is_some() && details.get("resourcePath").is_some()
            });
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("code", &self.code)?;
        map.serialize_entry("message", &self.message)?;
        if let Some(context) = resource_context {
            map.serialize_entry("resourceKind", &context["resourceKind"])?;
            map.serialize_entry("resourcePath", &context["resourcePath"])?;
            if let Some(recovery_required) = context.get("recoveryRequired") {
                map.serialize_entry("recoveryRequired", recovery_required)?;
            }
        } else if let Some(details) = &self.details {
            map.serialize_entry("details", details)?;
        }
        map.end()
    }
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

impl From<ProjectFilesystemError> for AppError {
    fn from(error: ProjectFilesystemError) -> Self {
        let mut app_error = Self::new(error.code(), error.to_string());
        if error.recovery_required() {
            app_error.details = Some(serde_json::json!({ "recoveryRequired": true }));
        }
        app_error
    }
}

impl From<ProjectError> for AppError {
    fn from(error: ProjectError) -> Self {
        let code = match error {
            ProjectError::FileNotFound(_) => "project_not_found",
            ProjectError::InvalidProjectFormat(_) => "invalid_project_format",
            ProjectError::InvalidGraphDocument { .. } => "invalid_graph_document",
            ProjectError::Serialize(_) => "project_serialize_failed",
            ProjectError::Deserialize(_) => "project_deserialize_failed",
            ProjectError::Io(_) => "project_io_failed",
        };
        Self::new(code, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, GraphMutationErrorDetailsDto};

    #[test]
    fn serializes_stable_ipc_error_shape() {
        let value = serde_json::to_value(AppError::new("project_not_found", "missing")).unwrap();
        assert_eq!(value["code"], "project_not_found");
        assert_eq!(value["message"], "missing");
        assert!(value.get("details").is_none());
    }

    #[test]
    fn phase1_error_protocol_graph_mutation_details_are_stable() {
        assert_eq!(
            serde_json::to_value(GraphMutationErrorDetailsDto::VALUE).unwrap(),
            serde_json::json!({ "category": "graphMutation" })
        );
    }
}
