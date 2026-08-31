use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JuliaWorkerErrorCode {
    RuntimeUnavailable,
    EnvironmentUnavailable,
    StartFailed,
    RequestFailed,
    ResponseTimeout,
    InvalidResponse,
    StateUnavailable,
    AssetUpdateFailed,
    TaskDirectoryInvalid,
    TaskDirectoryCreateFailed,
    TaskDirectoryCleanupFailed,
    InputWriteFailed,
    ModelGenerationFailed,
    TaskGenerationFailed,
    InvalidRequest,
    InvalidParameters,
    UnsupportedCapability,
    PackageUnavailable,
    SamplingFailed,
    Cancelled,
    Internal,
}

impl JuliaWorkerErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeUnavailable => "julia_worker_runtime_unavailable",
            Self::EnvironmentUnavailable => "julia_worker_environment_unavailable",
            Self::StartFailed => "julia_worker_start_failed",
            Self::RequestFailed => "julia_worker_request_failed",
            Self::ResponseTimeout => "julia_worker_response_timeout",
            Self::InvalidResponse => "julia_worker_invalid_response",
            Self::StateUnavailable => "julia_worker_state_unavailable",
            Self::AssetUpdateFailed => "julia_worker_asset_update_failed",
            Self::TaskDirectoryInvalid => "julia_worker_task_directory_invalid",
            Self::TaskDirectoryCreateFailed => "julia_worker_task_directory_create_failed",
            Self::TaskDirectoryCleanupFailed => "julia_worker_task_directory_cleanup_failed",
            Self::InputWriteFailed => "julia_worker_input_write_failed",
            Self::ModelGenerationFailed => "julia_worker_model_generation_failed",
            Self::TaskGenerationFailed => "julia_worker_task_generation_failed",
            Self::InvalidRequest => "julia_worker_invalid_request",
            Self::InvalidParameters => "julia_worker_invalid_parameters",
            Self::UnsupportedCapability => "julia_worker_unsupported_capability",
            Self::PackageUnavailable => "julia_worker_package_unavailable",
            Self::SamplingFailed => "julia_worker_sampling_failed",
            Self::Cancelled => "julia_worker_cancelled",
            Self::Internal => "julia_worker_internal_error",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JuliaWorkerErrorDetails {
    pub column: Option<String>,
    pub row: Option<usize>,
    pub parameter: Option<String>,
    pub path: Option<String>,
}

impl JuliaWorkerErrorDetails {
    fn from_value(value: Option<&Value>) -> Option<Self> {
        let value = value?;
        let details = Self {
            column: safe_detail(value, "column"),
            row: value
                .get("row")
                .and_then(Value::as_u64)
                .and_then(|row| usize::try_from(row).ok()),
            parameter: safe_detail(value, "parameter"),
            path: safe_detail(value, "path"),
        };
        (details != Self::default()).then_some(details)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JuliaWorkerError {
    code: JuliaWorkerErrorCode,
    details: Option<JuliaWorkerErrorDetails>,
    diagnostic: String,
}

impl JuliaWorkerError {
    /// Creates a worker failure with a stable code and private diagnostic detail.
    pub fn new(code: JuliaWorkerErrorCode, diagnostic: impl Into<String>) -> Self {
        Self {
            code,
            details: None,
            diagnostic: diagnostic.into(),
        }
    }

    /// Maps a Julia JSON-RPC error payload into a stable worker failure.
    pub fn from_json_rpc_error(error: &Value) -> Self {
        let code = match error.get("code").and_then(Value::as_str) {
            Some("invalid_request") => JuliaWorkerErrorCode::InvalidRequest,
            Some("invalid_parameters") => JuliaWorkerErrorCode::InvalidParameters,
            Some("unsupported_capability") => JuliaWorkerErrorCode::UnsupportedCapability,
            Some("package_unavailable") => JuliaWorkerErrorCode::PackageUnavailable,
            Some("sampling_failed") => JuliaWorkerErrorCode::SamplingFailed,
            Some("cancelled") => JuliaWorkerErrorCode::Cancelled,
            Some("internal_error") | None => JuliaWorkerErrorCode::Internal,
            Some(_) => JuliaWorkerErrorCode::Internal,
        };
        let diagnostic = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Julia worker task failed.")
            .to_string();
        Self {
            code,
            details: JuliaWorkerErrorDetails::from_value(error.get("data")),
            diagnostic,
        }
    }

    pub fn code(&self) -> JuliaWorkerErrorCode {
        self.code
    }

    pub fn details(&self) -> Option<&JuliaWorkerErrorDetails> {
        self.details.as_ref()
    }

    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
}

impl std::fmt::Display for JuliaWorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code.as_str())
    }
}

impl std::error::Error for JuliaWorkerError {}

fn safe_detail(value: &Value, field: &str) -> Option<String> {
    let value = value.get(field)?.as_str()?;
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.to_string())
}
