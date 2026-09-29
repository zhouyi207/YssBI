//! Provider configuration and runtime model selection.

use crate::driver::RigAgentDriver;
use crate::error::{RigProviderConfigurationError, provider_unavailable};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use yss_harness_contract::{
    AgentDriverConfigurationFailure, AgentDriverConfigurationPort, AgentDriverFailure,
    AgentDriverPort, AgentEventOutput, AgentTurnRequest, AgentTurnResult, CancellationToken,
    ModelCapabilityExecutor, SecretCredential,
};

use rig_core::{client::CompletionClient, providers::openai};

pub fn openai_agent_driver(
    api_key: yss_harness_contract::SecretCredential,
    base_url: impl Into<String>,
    model: impl Into<String>,
) -> Result<Arc<dyn AgentDriverPort>, RigProviderConfigurationError> {
    let http_client = rig_core::http_client::ReqwestClient::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| RigProviderConfigurationError::Invalid)?;
    openai_agent_driver_with_client(api_key, base_url, model, http_client)
}

pub(crate) fn openai_agent_driver_with_client(
    api_key: SecretCredential,
    base_url: impl Into<String>,
    model: impl Into<String>,
    http_client: rig_core::http_client::ReqwestClient,
) -> Result<Arc<dyn AgentDriverPort>, RigProviderConfigurationError> {
    let base_url = base_url.into();
    let model = model.into();
    if !is_valid_base_url(&base_url) || model.trim().is_empty() || model.len() > 256 {
        return Err(RigProviderConfigurationError::Invalid);
    }
    let client = openai::Client::builder()
        .api_key(api_key.expose())
        .base_url(base_url)
        .http_client(http_client)
        .build()
        .map_err(|_| RigProviderConfigurationError::Invalid)?
        // Rig defaults to Responses; configured compatible services use Chat Completions.
        .completions_api();
    let driver = RigAgentDriver::new(client.completion_model(model));
    Ok(Arc::new(driver))
}

/// Runtime-switchable provider adapter. The Harness keeps this stable for the
/// lifetime of the application while settings commands replace only the model
/// driver used by newly admitted turns.
#[derive(Default)]
pub struct ConfigurableAgentDriver {
    driver: RwLock<Option<Arc<dyn AgentDriverPort>>>,
}

impl ConfigurableAgentDriver {
    pub fn new() -> Self {
        Self::default()
    }

    fn set_unavailable(&self) {
        *self
            .driver
            .write()
            .unwrap_or_else(|error| error.into_inner()) = None;
    }
}

impl AgentDriverPort for ConfigurableAgentDriver {
    fn run_turn<'a>(
        &'a self,
        request: AgentTurnRequest,
        capabilities: Arc<dyn ModelCapabilityExecutor>,
        output: Arc<dyn AgentEventOutput>,
        cancellation: CancellationToken,
    ) -> yss_harness_contract::AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        let driver = self
            .driver
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        Box::pin(async move {
            driver
                .ok_or_else(provider_unavailable)?
                .run_turn(request, capabilities, output, cancellation)
                .await
        })
    }
}

impl AgentDriverConfigurationPort for ConfigurableAgentDriver {
    fn configure(
        &self,
        base_url: String,
        model: String,
        credential: Option<SecretCredential>,
    ) -> Result<bool, AgentDriverConfigurationFailure> {
        let Some(credential) = credential else {
            self.set_unavailable();
            return Ok(false);
        };
        if model.trim().is_empty() {
            self.set_unavailable();
            return Ok(false);
        }

        let driver = match openai_agent_driver(credential, base_url, model) {
            Ok(driver) => driver,
            Err(_) => {
                self.set_unavailable();
                return Err(AgentDriverConfigurationFailure::Invalid);
            }
        };
        *self
            .driver
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(driver);
        Ok(true)
    }

    fn is_configured(&self) -> bool {
        self.driver
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .is_some()
    }
}

fn is_valid_base_url(base_url: &str) -> bool {
    let trimmed = base_url.trim();
    (trimmed.starts_with("https://") || trimmed.starts_with("http://"))
        && trimmed.len() <= 2_048
        && !trimmed.chars().any(char::is_whitespace)
}
