//! Stable error classification without forwarding provider response content.
use rig_agent::completion::PromptError;
use rig_core::error::{ErrorKind, ErrorReport, ProviderError};
use yss_harness_contract::{AgentDriverFailure, AgentDriverFailureCode};

pub(crate) fn map_prompt_failure(error: PromptError) -> AgentDriverFailure {
    use AgentDriverFailureCode::*;
    match error {
        PromptError::CompletionError(error) => map_provider_failure(error),
        PromptError::Report(report) => map_report(&report),
        PromptError::PromptCancelled { .. } => cancelled(),
        PromptError::UnknownToolCall { .. } => invalid_response(),
        PromptError::MaxTurnsError { .. } | PromptError::MemoryError(_) => {
            AgentDriverFailure::new(InternalFailure)
        }
    }
}

pub(crate) fn map_provider_failure(error: ProviderError) -> AgentDriverFailure {
    map_report(&error.report())
}

fn map_report(report: &ErrorReport) -> AgentDriverFailure {
    use AgentDriverFailureCode::*;
    let retry_after_ms = report
        .provider_response_headers()
        .and_then(|headers| headers.get("retry-after"))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1_000));
    let code = if matches!(
        report.code.as_deref(),
        Some("context_length_exceeded" | "context_window_exceeded")
    ) {
        ContextWindowExceeded
    } else {
        match report.http_status {
            Some(401 | 403) => ProviderAuthenticationFailed,
            Some(402) => ProviderPaymentRequired,
            Some(429) => ProviderRateLimited,
            Some(408 | 504) => ProviderTransportFailed,
            Some(300..=499) => ProviderRequestRejected,
            Some(500..=599) => ProviderUnavailable,
            Some(_) => InvalidProviderResponse,
            None => match report.kind {
                ErrorKind::Http | ErrorKind::Timeout => ProviderTransportFailed,
                ErrorKind::Url | ErrorKind::Request => ProviderRequestRejected,
                ErrorKind::Cancelled => Cancelled,
                ErrorKind::Response if report.retryable => ProviderStreamInterrupted,
                ErrorKind::Response | ErrorKind::Json | ErrorKind::ProviderResponse => {
                    InvalidProviderResponse
                }
                ErrorKind::Provider => ProviderUnavailable,
                _ => InternalFailure,
            },
        }
    };
    // Reports also contain response bodies and source chains; log only structural facts.
    tracing::warn!(domain = "Application", event = "harness_provider_failure",
        category = report.kind.code(), http_status = report.http_status, failure_code = %code,
        "Harness provider turn failed");
    AgentDriverFailure {
        code,
        retry_after_ms,
    }
}

pub(crate) fn cancelled() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::Cancelled)
}
pub(crate) fn invalid_response() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::InvalidProviderResponse)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RigProviderConfigurationError {
    #[error("Rig provider configuration is invalid")]
    Invalid,
}
