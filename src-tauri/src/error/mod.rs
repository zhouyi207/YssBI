use std::fmt;

use serde::Serialize;
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::project::{ProjectDatabaseError, ProjectError, ProjectFilesystemError};

pub(crate) fn new_diagnostic_incident_id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMutationErrorDetailsDto {
    pub category: &'static str,
}

impl GraphMutationErrorDetailsDto {
    pub const VALUE: Self = Self {
        category: "graphMutation",
    };
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    code: &'static str,
    details: Value,
    incident_id: Option<String>,
}

impl CommandError {
    pub fn expected(code: &'static str) -> Self {
        assert_valid_code(code);
        Self {
            code,
            details: Value::Null,
            incident_id: None,
        }
    }

    pub fn internal(error: impl fmt::Display + fmt::Debug) -> Self {
        Self::record_incident("internal_error", error)
    }

    pub fn diagnosed(code: &'static str, error: impl fmt::Display + fmt::Debug) -> Self {
        assert_valid_code(code);
        Self::record_incident(code, error)
    }

    pub fn with_details<T: Serialize>(mut self, details: T) -> Self {
        match serde_json::to_value(details) {
            Ok(value @ (Value::Object(_) | Value::Null)) => {
                self.details = value;
                self
            }
            Ok(value) => Self::internal(format!(
                "details for command error '{}' serialized as {}, expected object or null",
                self.code,
                value_kind(&value)
            )),
            Err(error) => Self::internal(format!(
                "failed to serialize details for command error '{}': {error}",
                self.code
            )),
        }
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub fn details(&self) -> Option<&Map<String, Value>> {
        self.details.as_object()
    }

    pub fn incident_id(&self) -> Option<&str> {
        self.incident_id.as_deref()
    }

    fn record_incident(code: &'static str, error: impl fmt::Display + fmt::Debug) -> Self {
        let incident_id = new_diagnostic_incident_id();
        tracing::error!(
            target: "yssbi::command_error",
            diagnostic_domain = "application",
            diagnostic_event = "commandError",
            error_code = code,
            incident_id = incident_id.as_str(),
            error = %error,
            error_debug = ?error,
            "Command failed"
        );
        Self {
            code,
            details: Value::Null,
            incident_id: Some(incident_id),
        }
    }
}

impl From<ProjectFilesystemError> for CommandError {
    fn from(error: ProjectFilesystemError) -> Self {
        let recovery_required = error.recovery_required();
        let command_error = Self::expected(error.code());
        if recovery_required {
            command_error.with_details(RecoveryRequiredDetails {
                recovery_required: true,
            })
        } else {
            command_error
        }
    }
}

impl From<ProjectDatabaseError> for CommandError {
    fn from(error: ProjectDatabaseError) -> Self {
        let Some(code) = error.command_code() else {
            return Self::internal(error);
        };
        let recovery_required = error.recovery_required();
        let command_error = Self::expected(code);
        if recovery_required {
            command_error.with_details(RecoveryRequiredDetails {
                recovery_required: true,
            })
        } else {
            command_error
        }
    }
}

impl From<ProjectError> for CommandError {
    fn from(error: ProjectError) -> Self {
        match error {
            ProjectError::FileNotFound(_) => Self::expected("project_not_found"),
            ProjectError::InvalidProjectFormat(_) => Self::expected("invalid_project_format"),
            ProjectError::InvalidGraphDocument { .. } => Self::expected("invalid_graph_document"),
            error @ ProjectError::Serialize(_) => {
                Self::diagnosed("project_serialize_failed", error)
            }
            error @ ProjectError::Deserialize(_) => {
                Self::diagnosed("project_deserialize_failed", error)
            }
            error @ ProjectError::Io(_) => Self::diagnosed("project_io_failed", error),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryRequiredDetails {
    recovery_required: bool,
}

fn assert_valid_code(code: &str) {
    let mut bytes = code.bytes();
    let starts_lowercase = bytes.next().is_some_and(|byte| byte.is_ascii_lowercase());
    let remaining_valid =
        bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    assert!(
        starts_lowercase && remaining_valid && !code.ends_with('_') && !code.contains("__"),
        "command error code must be lower_snake_case: {code}"
    );
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_json::json;
    use tracing_subscriber::layer::SubscriberExt;
    use uuid::Uuid;

    use super::{CommandError, GraphMutationErrorDetailsDto};
    use yss_diagnostics::DiagnosticsRuntime;
    use yss_tracing::LogLayer;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct TestDetails {
        resource_path: &'static str,
    }

    #[test]
    fn expected_error_serializes_exact_wire_keys_with_object_details() {
        let error = CommandError::expected("resource_not_found").with_details(TestDetails {
            resource_path: "events/Missing.yssbi-event",
        });

        assert_eq!(
            serde_json::to_value(error).unwrap(),
            json!({
                "code": "resource_not_found",
                "details": { "resourcePath": "events/Missing.yssbi-event" },
                "incidentId": null,
            })
        );
    }

    #[test]
    fn expected_error_without_details_serializes_explicit_nulls() {
        assert_eq!(
            serde_json::to_value(CommandError::expected("project_not_found")).unwrap(),
            json!({
                "code": "project_not_found",
                "details": null,
                "incidentId": null,
            })
        );
    }

    #[test]
    fn internal_error_wire_does_not_leak_diagnostic_message() {
        let error = CommandError::internal("secret database password");
        let value = serde_json::to_value(&error).unwrap();

        assert_eq!(value.as_object().unwrap().len(), 3);
        assert_eq!(value["code"], "internal_error");
        assert!(value["details"].is_null());
        assert!(Uuid::parse_str(value["incidentId"].as_str().unwrap()).is_ok());
        assert!(value.get("message").is_none());
        assert!(!value.to_string().contains("secret database password"));
    }

    #[test]
    fn internal_error_logs_incident_and_full_diagnostic_fields() {
        let diagnostics = DiagnosticsRuntime::initialize().expect("initialize diagnostics");
        let subscriber =
            tracing_subscriber::registry().with(LogLayer::new(diagnostics.rust_log_sink()));
        let error = tracing::subscriber::with_default(subscriber, || {
            CommandError::internal("full internal diagnostic")
        });

        let subscription = diagnostics.subscribe_batches(|_| true).unwrap();
        let record = subscription.entries.last().unwrap();
        assert_eq!(record.target, "yssbi::command_error");
        assert_eq!(record.event.as_deref(), Some("commandError"));
        assert_eq!(record.fields["error_code"], "internal_error");
        assert_eq!(record.fields["incident_id"], error.incident_id().unwrap());
        assert_eq!(record.fields["error"], "full internal diagnostic");
        assert_eq!(record.fields["error_debug"], "\"full internal diagnostic\"");
        diagnostics
            .unsubscribe(subscription.subscription_id)
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "lower_snake_case")]
    fn rejects_non_lower_snake_case_codes() {
        let _ = CommandError::expected("invalid-Code");
    }

    #[test]
    fn graph_mutation_details_are_stable() {
        assert_eq!(
            serde_json::to_value(GraphMutationErrorDetailsDto::VALUE).unwrap(),
            json!({ "category": "graphMutation" })
        );
    }
}
