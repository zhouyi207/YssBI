//! Translate per-turn effort choices at the provider boundary.
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentEvent, LanguageModelConfig,
    LanguageModelProtocol, ModelCallPurpose, ModelTokenUsage, ReasoningEffort,
};

fn effort_field(protocol: LanguageModelProtocol) -> (Option<&'static str>, &'static str) {
    match protocol {
        LanguageModelProtocol::OpenAiResponses => (Some("reasoning"), "effort"),
        LanguageModelProtocol::OpenAiChat => (None, "reasoning_effort"),
        LanguageModelProtocol::Anthropic => (Some("output_config"), "effort"),
        LanguageModelProtocol::Gemini => (Some("generation_config"), "thinking_level"),
    }
}

/// The configured default is known only when native generation parameters declare it.
pub fn model_default_reasoning_effort(
    config: &LanguageModelConfig,
    protocol: LanguageModelProtocol,
) -> Option<ReasoningEffort> {
    let (object, key) = effort_field(protocol);
    let parameters = match object {
        Some(object) => config.additional_parameters.get(object)?.as_object()?,
        None => &config.additional_parameters,
    };
    serde_json::from_value(parameters.get(key)?.clone()).ok()
}

pub(crate) fn reasoning_parameters(
    mut parameters: serde_json::Value,
    protocol: LanguageModelProtocol,
    configured: &[ReasoningEffort],
    effort: Option<ReasoningEffort>,
) -> Result<serde_json::Value, AgentDriverFailure> {
    let Some(effort) = effort else {
        return Ok(parameters);
    };
    if !configured.is_empty() && !configured.contains(&effort) {
        return Err(AgentDriverFailure::new(
            AgentDriverFailureCode::ProviderRequestRejected,
        ));
    }
    let value = match effort {
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
    };
    let (object, key) = effort_field(protocol);
    let target = if let Some(object) = object {
        let fields = parameters.as_object_mut().ok_or_else(rejected)?;
        fields
            .entry(object)
            .or_insert_with(|| serde_json::json!({}))
    } else {
        &mut parameters
    };
    target
        .as_object_mut()
        .ok_or_else(rejected)?
        .insert(key.into(), value.into());
    Ok(parameters)
}

fn rejected() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::ProviderRequestRejected)
}

pub(crate) fn usage_event(
    usage: rig_core::completion::Usage,
    context_window: Option<u32>,
    purpose: ModelCallPurpose,
) -> AgentEvent {
    AgentEvent::UsageReported {
        usage: ModelTokenUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cached_input_tokens: usage.cached_input_tokens,
            cache_creation_input_tokens: usage.cache_creation_input_tokens,
            reasoning_tokens: usage.reasoning_tokens,
        },
        context_window,
        purpose,
    }
}
