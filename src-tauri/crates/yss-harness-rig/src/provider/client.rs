//! Native Rig providers share the same Harness loop, tools and recovery hooks.

use crate::{RigAgentDriver, RigProviderConfigurationError};
use rig_core::http_client::{DynHttpClient, Uri};
use rig_core::providers::openai::{Route, wire::Auth};
use rig_core::providers::registry::{Format, ProviderConfig, ProviderId};
use rig_reqwest::{ReqwestClient, reqwest};
use std::sync::Arc;
use std::time::Duration;
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentDriverPort, LanguageModelAuthentication,
    LanguageModelConfig, LanguageModelProtocol, LanguageModelProviderConfig, SecretCredential,
};

pub struct RigProviderClient {
    protocol: LanguageModelProtocol,
    config: ProviderConfig,
    http: DynHttpClient,
}

impl RigProviderClient {
    pub fn new(
        settings: &LanguageModelProviderConfig,
        credential: Option<&SecretCredential>,
    ) -> Result<Self, RigProviderConfigurationError> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            // Redirects must not forward prompts or provider-specific credentials
            // to an endpoint the user did not configure.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| RigProviderConfigurationError::Invalid)?;
        Self::with_http_client(settings, credential, client)
    }

    pub(crate) fn with_http_client(
        settings: &LanguageModelProviderConfig,
        credential: Option<&SecretCredential>,
        client: reqwest::Client,
    ) -> Result<Self, RigProviderConfigurationError> {
        let protocol = settings.protocol;
        if !settings.validate() || !valid_base_url(&settings.base_url) {
            return Err(RigProviderConfigurationError::Invalid);
        }
        let format = match protocol {
            LanguageModelProtocol::OpenAiResponses | LanguageModelProtocol::OpenAiChat => {
                Format::OpenAi
            }
            LanguageModelProtocol::Anthropic => Format::Anthropic,
            LanguageModelProtocol::Gemini => Format::Gemini,
        };
        let adapter = ProviderId::resolve(&settings.adapter)
            .map_err(|_| RigProviderConfigurationError::Invalid)?;
        if adapter.format() != format {
            return Err(RigProviderConfigurationError::Invalid);
        }
        let key = match settings.authentication {
            LanguageModelAuthentication::ApiKey => credential
                .ok_or(RigProviderConfigurationError::Invalid)?
                .expose(),
            LanguageModelAuthentication::None => "",
        };
        let config = adapter.config(key);
        let base_url = settings.base_url.trim_end_matches('/');
        let config = match config {
            ProviderConfig::OpenAi(mut config) => {
                if settings.authentication == LanguageModelAuthentication::None {
                    config = config.with_auth(Auth::OptionalBearer);
                }
                ProviderConfig::OpenAi(config.with_base_url(base_url).with_route(
                    if protocol == LanguageModelProtocol::OpenAiResponses {
                        Route::Responses
                    } else {
                        Route::Chat
                    },
                ))
            }
            ProviderConfig::Anthropic(config) => {
                ProviderConfig::Anthropic(config.with_base_url(base_url))
            }
            ProviderConfig::Gemini(config) => {
                ProviderConfig::Gemini(config.with_base_url(base_url))
            }
        };
        Ok(Self {
            protocol,
            config,
            http: DynHttpClient::new(ReqwestClient::from(client)),
        })
    }

    pub fn driver(
        &self,
        model: &LanguageModelConfig,
    ) -> Result<Arc<dyn AgentDriverPort>, RigProviderConfigurationError> {
        if !model.validate(self.protocol) {
            return Err(RigProviderConfigurationError::Invalid);
        }
        let completion = match &self.config {
            ProviderConfig::OpenAi(config) => config
                .clone()
                .connect(self.http.clone())
                .completion(&model.id)
                .erase(),
            ProviderConfig::Anthropic(config) => config
                .clone()
                .connect(self.http.clone())
                .completion(&model.id)
                .erase(),
            // GenerateContent's Rig schema mapper drops union constraints.
            // Interactions retains the complete JSON Schema of Harness tools.
            ProviderConfig::Gemini(config) => rig_core::driver::Model::new(
                rig_core::providers::gemini::interactions_api::Interactions::new(
                    config.clone(),
                    &model.id,
                ),
                self.http.clone(),
            )
            .erase(),
        };
        Ok(Arc::new(
            RigAgentDriver::new(completion).with_model_config(model, self.protocol),
        ))
    }

    pub async fn list_models(&self) -> Result<Vec<LanguageModelConfig>, AgentDriverFailure> {
        let listing = async {
            match &self.config {
                ProviderConfig::OpenAi(config) => {
                    config
                        .clone()
                        .connect(self.http.clone())
                        .list_models()
                        .await
                }
                ProviderConfig::Anthropic(config) => {
                    config
                        .clone()
                        .connect(self.http.clone())
                        .list_models()
                        .await
                }
                ProviderConfig::Gemini(config) => {
                    config
                        .clone()
                        .connect(self.http.clone())
                        .list_models()
                        .await
                }
            }
        };
        let models = tokio::time::timeout(Duration::from_secs(30), listing)
            .await
            .map_err(|_| AgentDriverFailure::new(AgentDriverFailureCode::ProviderTransportFailed))?
            .map_err(crate::error::map_provider_failure)?;
        let mut models: Vec<_> = models
            .iter()
            .filter(|model| !model.id.trim().is_empty())
            .filter(|model| !matches!(model.r#type.as_deref(), Some("embedding" | "embeddings")))
            .map(|model| LanguageModelConfig {
                id: model.id.clone(),
                name: model.display_name().to_owned(),
                context_window: model.context_length,
                max_output_tokens: model.max_output_tokens,
                temperature: None,
                top_p: None,
                additional_parameters: Default::default(),
            })
            .collect();
        models.sort_by(|left, right| left.id.cmp(&right.id));
        models.dedup_by(|left, right| left.id == right.id);
        Ok(models)
    }
}

fn valid_base_url(base_url: &str) -> bool {
    let Ok(uri) = base_url.parse::<Uri>() else {
        return false;
    };
    matches!(uri.scheme_str(), Some("http" | "https"))
        && uri.host().is_some_and(|host| !host.is_empty())
        && uri
            .authority()
            .is_some_and(|authority| !authority.as_str().contains('@'))
        && uri.query().is_none()
        && !base_url.contains('#')
        && !base_url.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests;
