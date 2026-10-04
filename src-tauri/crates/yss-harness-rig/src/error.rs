//! Stable error classification without forwarding provider response content.

use rig_agent::completion::PromptError;
use rig_core::completion::CompletionError;
use yss_harness_contract::{AgentDriverFailure, AgentDriverFailureCode};

pub(crate) fn map_prompt_failure(error: PromptError) -> AgentDriverFailure {
    use AgentDriverFailureCode::*;
    // Provider errors can contain credentials, model output or tool arguments.
    // Only record structural categories and positions, never their Display/Debug text.
    let category = match &error {
        PromptError::CompletionError(completion) => match completion {
            CompletionError::HttpError(_) => "http",
            CompletionError::UrlError(_) => "url",
            CompletionError::RequestError(_) => "request",
            CompletionError::JsonError(json) => {
                tracing::warn!(
                    domain = "Application", event = "harness_response_json_failure",
                    json_category = ?json.classify(), line = json.line(), column = json.column(),
                    "Harness could not decode provider JSON"
                );
                "json"
            }
            CompletionError::ResponseError(_) => "response",
            CompletionError::ProviderResponse(_) => "provider_response",
            CompletionError::ProviderError(_) => "provider",
        },
        PromptError::UnknownToolCall { .. } => "unknown_tool_call",
        PromptError::PromptCancelled { .. } => "cancelled",
        PromptError::MemoryError(_) => "memory",
        PromptError::MaxTurnsError { .. } => "max_turns",
    };
    let http_status = error
        .provider_response_status()
        .map(|status| status.as_u16());
    let context_exceeded = error
        .provider_response_json()
        .ok()
        .flatten()
        .is_some_and(|body| {
            body.pointer("/error/code")
                .and_then(serde_json::Value::as_str)
                == Some("context_length_exceeded")
        });
    let code = if context_exceeded {
        ContextWindowExceeded
    } else {
        match http_status {
            Some(401 | 403) => ProviderAuthenticationFailed,
            Some(402) => ProviderPaymentRequired,
            Some(429) => ProviderRateLimited,
            Some(408 | 504) => ProviderTransportFailed,
            Some(400..=499) => ProviderRequestRejected,
            Some(500..=599) => ProviderUnavailable,
            Some(_) => InvalidProviderResponse,
            None => match error {
                PromptError::CompletionError(CompletionError::HttpError(_)) => {
                    ProviderTransportFailed
                }
                PromptError::CompletionError(
                    CompletionError::UrlError(_) | CompletionError::RequestError(_),
                ) => ProviderRequestRejected,
                PromptError::CompletionError(
                    CompletionError::JsonError(_) | CompletionError::ProviderResponse(_),
                )
                | PromptError::UnknownToolCall { .. } => InvalidProviderResponse,
                PromptError::CompletionError(CompletionError::ResponseError(message)) => {
                    response_failure(&message)
                }
                PromptError::CompletionError(CompletionError::ProviderError(_)) => {
                    ProviderUnavailable
                }
                PromptError::PromptCancelled { .. } => Cancelled,
                PromptError::MaxTurnsError { .. } => InternalFailure,
                PromptError::MemoryError(_) => InternalFailure,
            },
        }
    };
    tracing::warn!(
        domain = "Application", event = "harness_provider_failure",
        category, http_status, failure_code = %code,
        "Harness provider turn failed"
    );
    AgentDriverFailure::new(code)
}

fn response_failure(message: &str) -> AgentDriverFailureCode {
    use AgentDriverFailureCode::*;
    // Rig 0.42 reports these assembly failures as strings. Classify only its
    // known structural messages; never log the error's arbitrary response text.
    if message == "provider stream ended without a terminal record; treating the turn as truncated"
    {
        ProviderStreamInterrupted
    } else if message
        .starts_with("the model produced no answer and stopped with finish_reason=Length;")
    {
        ProviderOutputTruncated
    } else if message
        .starts_with("the model produced no answer and stopped with finish_reason=ContentFilter;")
    {
        ProviderContentFiltered
    } else {
        InvalidProviderResponse
    }
}

pub(crate) fn cancelled() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::Cancelled)
}

pub(crate) fn provider_unavailable() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::ProviderUnavailable)
}

pub(crate) fn invalid_response() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::InvalidProviderResponse)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RigProviderConfigurationError {
    #[error("Rig provider configuration is invalid")]
    Invalid,
}
