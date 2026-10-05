//! Provider configuration and model identity, independent of any inference SDK.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageModelProtocol {
    OpenAiResponses,
    OpenAiChat,
    Anthropic,
    Gemini,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageModelAuthentication {
    ApiKey,
    None,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageModelConfig {
    pub id: String,
    pub name: String,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub additional_parameters: serde_json::Map<String, serde_json::Value>,
}

impl LanguageModelConfig {
    pub fn validate(&self, protocol: LanguageModelProtocol) -> bool {
        !self.id.trim().is_empty()
            && !self.name.trim().is_empty()
            && !self.id.chars().any(char::is_control)
            && self.context_window != Some(0)
            && self.max_output_tokens != Some(0)
            && self.temperature.is_none_or(|value| value.is_finite() && (0.0..=if protocol == LanguageModelProtocol::Anthropic { 1.0 } else { 2.0 }).contains(&value))
            && self.top_p.is_none_or(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            && self.additional_parameters.keys().all(|key| !reserved_parameter(key))
            && ["generationConfig", "generation_config"].iter().all(|key| {
                self.additional_parameters.get(*key).is_none_or(|value| {
                    value.as_object().is_some_and(|fields| fields.keys().all(|field| !reserved_parameter(field)))
                })
            })
            && !matches!((self.context_window, self.max_output_tokens), (Some(context), Some(output)) if output >= context)
            // Anthropic requires max_tokens even for models unknown to the SDK.
            && (protocol != LanguageModelProtocol::Anthropic || self.max_output_tokens.is_some())
    }
}

// These fields belong to the Harness request, authentication, or the explicit
// sampling controls. Provider extensions may not replace that request envelope.
fn reserved_parameter(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        normalized.as_str(),
        "model"
            | "messages"
            | "input"
            | "contents"
            | "system"
            | "systeminstruction"
            | "instructions"
            | "tools"
            | "toolchoice"
            | "toolconfig"
            | "functions"
            | "functioncall"
            | "stream"
            | "streamoptions"
            | "apikey"
            | "authorization"
            | "headers"
            | "baseurl"
            | "endpoint"
            | "cachedcontent"
            | "previousresponseid"
            | "previousinteractionid"
            | "agent"
            | "agentconfig"
            | "conversation"
            | "background"
            | "store"
            | "n"
            | "candidatecount"
            | "temperature"
            | "topp"
            | "maxtokens"
            | "maxoutputtokens"
            | "maxcompletiontokens"
    )
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageModelProviderConfig {
    pub id: String,
    pub name: String,
    pub custom_name: Option<String>,
    pub protocol: LanguageModelProtocol,
    pub adapter: String,
    pub authentication: LanguageModelAuthentication,
    pub base_url: String,
    pub models: Vec<LanguageModelConfig>,
}

impl LanguageModelProviderConfig {
    pub fn validate(&self) -> bool {
        let mut ids = std::collections::BTreeSet::new();
        !self.id.trim().is_empty()
            && !self.name.trim().is_empty()
            && !self.adapter.trim().is_empty()
            && (self.authentication != LanguageModelAuthentication::None
                || matches!(
                    self.protocol,
                    LanguageModelProtocol::OpenAiChat | LanguageModelProtocol::OpenAiResponses
                ))
            && self
                .models
                .iter()
                .all(|model| model.validate(self.protocol) && ids.insert(model.id.as_str()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageModelSelection {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageModelIdentity {
    pub selection: LanguageModelSelection,
    pub provider_name: String,
    pub model_name: String,
}

pub struct ResolvedLanguageModel {
    pub identity: LanguageModelIdentity,
    pub driver: std::sync::Arc<dyn crate::AgentDriverPort>,
}

/// Resolve once at turn admission. Every agent in that turn shares this driver.
pub trait LanguageModelResolverPort: Send + Sync {
    fn resolve<'a>(
        &'a self,
        selection: Option<&'a LanguageModelSelection>,
    ) -> crate::AgentFuture<'a, Result<ResolvedLanguageModel, crate::AgentDriverFailure>>;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageModelProviderStatus {
    pub config: LanguageModelProviderConfig,
    pub has_api_key: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageModelProviderPreset {
    pub id: String,
    pub name: String,
    pub protocol: LanguageModelProtocol,
    pub adapter: String,
    pub authentication: LanguageModelAuthentication,
    pub base_url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageModelCatalog {
    pub presets: Vec<LanguageModelProviderPreset>,
    pub providers: Vec<LanguageModelProviderStatus>,
    pub default_model: Option<LanguageModelSelection>,
}
