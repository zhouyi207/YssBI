//! Application-owned model settings. Rig owns protocols; the OS owns secrets.

mod credentials;
pub use credentials::SystemModelCredentials;

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use yss_harness_contract::*;
use yss_harness_rig::RigProviderClient;

#[derive(Debug, thiserror::Error)]
pub enum ModelSettingsError {
    #[error("model configuration is invalid")]
    Invalid,
    #[error("model configuration is unavailable")]
    Unavailable,
    #[error("system credential store is unavailable")]
    Credentials,
    #[error("selected model is unavailable")]
    ModelUnavailable,
    #[error("provider request failed")]
    Provider(AgentDriverFailureCode),
}

pub trait ModelCredentials: Send + Sync {
    fn read(&self, key: &str) -> Result<Option<SecretCredential>, ModelSettingsError>;
    fn write(&self, key: &str, credential: &SecretCredential) -> Result<(), ModelSettingsError>;
    fn remove(&self, key: &str) -> Result<(), ModelSettingsError>;
}

pub enum CredentialChange {
    Keep,
    Replace(SecretCredential),
    Remove,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModelSettings {
    providers: Vec<StoredProvider>,
    default_model: Option<LanguageModelSelection>,
    // Credential entries are immutable. Retired entries remain reachable until
    // deletion succeeds, including across a crash after the settings commit.
    retired_credentials: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredProvider {
    config: LanguageModelProviderConfig,
    credential_key: Option<String>,
}

struct State {
    path: PathBuf,
    settings: ModelSettings,
    loaded: bool,
    credentials: Arc<dyn ModelCredentials>,
}

pub struct LanguageModelService {
    state: Arc<Mutex<State>>,
}

impl LanguageModelService {
    pub fn new(app_dir: PathBuf, credentials: Arc<dyn ModelCredentials>) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                path: app_dir.join("settings").join("language-models.json"),
                settings: ModelSettings::default(),
                loaded: false,
                credentials,
            })),
        }
    }

    async fn access<T: Send + 'static>(
        &self,
        action: impl FnOnce(&mut State) -> Result<T, ModelSettingsError> + Send + 'static,
    ) -> Result<T, ModelSettingsError> {
        let state = self.state.clone();
        tokio::task::spawn_blocking(move || {
            let mut state = state.lock().map_err(|_| ModelSettingsError::Unavailable)?;
            state.load()?;
            action(&mut state)
        })
        .await
        .map_err(|_| ModelSettingsError::Unavailable)?
    }

    pub async fn catalog(&self) -> Result<LanguageModelCatalog, ModelSettingsError> {
        self.access(|state| state.catalog()).await
    }

    pub async fn save_provider(
        &self,
        config: LanguageModelProviderConfig,
        credential: CredentialChange,
    ) -> Result<LanguageModelCatalog, ModelSettingsError> {
        self.access(move |state| {
            if !config.validate() {
                return Err(ModelSettingsError::Invalid);
            }
            // URL/protocol validation never opens a network connection.
            let probe = SecretCredential::new("configuration-validation")
                .map_err(|_| ModelSettingsError::Invalid)?;
            RigProviderClient::new(&config, Some(&probe))
                .map_err(|_| ModelSettingsError::Invalid)?;
            let mut candidate = state.settings.clone();
            let previous = candidate
                .providers
                .iter()
                .find(|entry| entry.config.id == config.id)
                .and_then(|entry| entry.credential_key.clone());
            let mut created_key = None;
            let credential = if config.authentication == LanguageModelAuthentication::None {
                CredentialChange::Remove
            } else {
                credential
            };
            let credential_key = match credential {
                CredentialChange::Keep => previous.clone(),
                CredentialChange::Remove => None,
                CredentialChange::Replace(value) => {
                    let key = uuid::Uuid::new_v4().to_string();
                    state.credentials.write(&key, &value)?;
                    created_key = Some(key.clone());
                    Some(key)
                }
            };
            if previous != credential_key
                && let Some(key) = previous
            {
                candidate.retired_credentials.push(key);
            }
            candidate
                .providers
                .retain(|entry| entry.config.id != config.id);
            candidate.providers.push(StoredProvider {
                config,
                credential_key,
            });
            candidate.providers.sort_by(|a, b| {
                a.config
                    .name
                    .cmp(&b.config.name)
                    .then(a.config.id.cmp(&b.config.id))
            });
            candidate.reconcile_default();
            if let Err(error) = state.commit(candidate) {
                if let Some(key) = created_key {
                    let _ = state.credentials.remove(&key);
                }
                return Err(error);
            }
            state.catalog()
        })
        .await
    }

    pub async fn delete_provider(
        &self,
        provider_id: String,
    ) -> Result<LanguageModelCatalog, ModelSettingsError> {
        self.access(move |state| {
            let mut candidate = state.settings.clone();
            if let Some(entry) = candidate
                .providers
                .iter()
                .find(|entry| entry.config.id == provider_id)
                && let Some(key) = &entry.credential_key
            {
                candidate.retired_credentials.push(key.clone());
            }
            candidate
                .providers
                .retain(|entry| entry.config.id != provider_id);
            candidate.reconcile_default();
            state.commit(candidate)?;
            state.catalog()
        })
        .await
    }

    pub async fn set_default(
        &self,
        selection: LanguageModelSelection,
    ) -> Result<LanguageModelCatalog, ModelSettingsError> {
        self.access(move |state| {
            state.settings.model(&selection)?;
            let mut candidate = state.settings.clone();
            candidate.default_model = Some(selection);
            state.commit(candidate)?;
            state.catalog()
        })
        .await
    }

    pub async fn discover_models(
        &self,
        mut config: LanguageModelProviderConfig,
        credential: Option<SecretCredential>,
    ) -> Result<Vec<LanguageModelConfig>, ModelSettingsError> {
        // Unfinished model forms must not prevent querying the connection draft.
        config.models.clear();
        let provider_id = config.id.clone();
        let (client, requested) = self
            .access(move |state| {
                let provider = state
                    .settings
                    .providers
                    .iter()
                    .find(|entry| entry.config.id == config.id);
                let credential = match config.authentication {
                    LanguageModelAuthentication::None => None,
                    LanguageModelAuthentication::ApiKey => Some(match credential {
                        Some(value) => value,
                        None => state
                            .credential(provider.ok_or(ModelSettingsError::ModelUnavailable)?)?
                            .ok_or(ModelSettingsError::ModelUnavailable)?,
                    }),
                };
                let client = RigProviderClient::new(&config, credential.as_ref())
                    .map_err(|_| ModelSettingsError::Invalid)?;
                Ok((client, provider.cloned()))
            })
            .await?;
        let models = client
            .list_models()
            .await
            .map_err(|error| ModelSettingsError::Provider(error.code))?;
        self.access(move |state| {
            // Drafts are never committed here. Recheck the saved baseline so
            // another window cannot replace or remove it during this request.
            let current = state
                .settings
                .providers
                .iter()
                .find(|provider| provider.config.id == provider_id);
            match (current, requested.as_ref()) {
                (None, None) => Ok(models),
                (Some(current), Some(requested))
                    if current.config.protocol == requested.config.protocol
                        && current.config.adapter == requested.config.adapter
                        && current.config.authentication == requested.config.authentication
                        && current.config.base_url == requested.config.base_url
                        && current.credential_key == requested.credential_key =>
                {
                    Ok(models)
                }
                _ => Err(ModelSettingsError::ModelUnavailable),
            }
        })
        .await
    }
}

impl LanguageModelResolverPort for LanguageModelService {
    fn resolve<'a>(
        &'a self,
        selection: Option<&'a LanguageModelSelection>,
    ) -> AgentFuture<'a, Result<ResolvedLanguageModel, AgentDriverFailure>> {
        let requested = selection.cloned();
        Box::pin(async move {
            self.access(move |state| {
                let selection = requested
                    .or_else(|| state.settings.default_model.clone())
                    .ok_or(ModelSettingsError::ModelUnavailable)?;
                let (provider, model) = state.settings.model(&selection)?;
                let driver = state
                    .client(provider)?
                    .driver(model)
                    .map_err(|_| ModelSettingsError::Invalid)?;
                Ok(ResolvedLanguageModel {
                    identity: LanguageModelIdentity {
                        selection,
                        provider_name: provider
                            .config
                            .custom_name
                            .as_deref()
                            .map(str::trim)
                            .filter(|name| !name.is_empty())
                            .unwrap_or(provider.config.name.trim())
                            .to_owned(),
                        model_name: model.name.clone(),
                    },
                    driver,
                })
            })
            .await
            .map_err(|error| {
                AgentDriverFailure::new(match error {
                    ModelSettingsError::Invalid => AgentDriverFailureCode::ProviderRequestRejected,
                    ModelSettingsError::Provider(code) => code,
                    _ => AgentDriverFailureCode::ProviderUnavailable,
                })
            })
        })
    }
}

impl ModelSettings {
    fn model(
        &self,
        selection: &LanguageModelSelection,
    ) -> Result<(&StoredProvider, &LanguageModelConfig), ModelSettingsError> {
        let provider = self
            .providers
            .iter()
            .find(|entry| entry.config.id == selection.provider_id)
            .ok_or(ModelSettingsError::ModelUnavailable)?;
        let model = provider
            .config
            .models
            .iter()
            .find(|model| model.id == selection.model_id)
            .ok_or(ModelSettingsError::ModelUnavailable)?;
        Ok((provider, model))
    }

    fn reconcile_default(&mut self) {
        if self
            .default_model
            .as_ref()
            .is_some_and(|selection| self.model(selection).is_ok())
        {
            return;
        }
        self.default_model = self.providers.iter().find_map(|entry| {
            entry
                .config
                .models
                .first()
                .map(|model| LanguageModelSelection {
                    provider_id: entry.config.id.clone(),
                    model_id: model.id.clone(),
                })
        });
    }
}

impl State {
    fn load(&mut self) -> Result<(), ModelSettingsError> {
        if self.loaded {
            return Ok(());
        }
        let settings: ModelSettings = match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| ModelSettingsError::Invalid)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => ModelSettings::default(),
            Err(_) => return Err(ModelSettingsError::Unavailable),
        };
        let mut ids = std::collections::BTreeSet::new();
        if settings
            .providers
            .iter()
            .any(|entry| !entry.config.validate() || !ids.insert(&entry.config.id))
        {
            return Err(ModelSettingsError::Invalid);
        }
        self.settings = settings;
        self.loaded = true;
        self.cleanup_credentials();
        Ok(())
    }

    fn catalog(&self) -> Result<LanguageModelCatalog, ModelSettingsError> {
        Ok(LanguageModelCatalog {
            presets: yss_harness_rig::provider_presets()
                .map_err(|_| ModelSettingsError::Invalid)?,
            providers: self
                .settings
                .providers
                .iter()
                .map(|entry| {
                    Ok(LanguageModelProviderStatus {
                        config: entry.config.clone(),
                        reasoning_defaults: entry
                            .config
                            .models
                            .iter()
                            .filter_map(|model| {
                                yss_harness_rig::model_default_reasoning_effort(
                                    model,
                                    entry.config.protocol,
                                )
                                .map(|effort| (model.id.clone(), effort))
                            })
                            .collect(),
                        has_api_key: match &entry.credential_key {
                            Some(key) => self.credentials.read(key)?.is_some(),
                            None => false,
                        },
                    })
                })
                .collect::<Result<_, ModelSettingsError>>()?,
            default_model: self.settings.default_model.clone(),
        })
    }

    fn credential(
        &self,
        provider: &StoredProvider,
    ) -> Result<Option<SecretCredential>, ModelSettingsError> {
        match provider.config.authentication {
            LanguageModelAuthentication::None => Ok(None),
            LanguageModelAuthentication::ApiKey => {
                let key = provider
                    .credential_key
                    .as_ref()
                    .ok_or(ModelSettingsError::ModelUnavailable)?;
                Ok(Some(
                    self.credentials
                        .read(key)?
                        .ok_or(ModelSettingsError::ModelUnavailable)?,
                ))
            }
        }
    }

    fn client(&self, provider: &StoredProvider) -> Result<RigProviderClient, ModelSettingsError> {
        let credential = self.credential(provider)?;
        RigProviderClient::new(&provider.config, credential.as_ref())
            .map_err(|_| ModelSettingsError::Invalid)
    }

    fn persist(&self, settings: &ModelSettings) -> Result<(), ModelSettingsError> {
        std::fs::create_dir_all(self.path.parent().ok_or(ModelSettingsError::Unavailable)?)
            .map_err(|_| ModelSettingsError::Unavailable)?;
        let bytes = serde_json::to_vec_pretty(settings).map_err(|_| ModelSettingsError::Invalid)?;
        atomicwrites::AtomicFile::new(&self.path, atomicwrites::AllowOverwrite)
            .write(|file| {
                file.write_all(&bytes)?;
                file.sync_all()
            })
            .map_err(|_| ModelSettingsError::Unavailable)
    }

    fn commit(&mut self, settings: ModelSettings) -> Result<(), ModelSettingsError> {
        self.persist(&settings)?;
        self.settings = settings;
        self.cleanup_credentials();
        Ok(())
    }

    fn cleanup_credentials(&mut self) {
        if self.settings.retired_credentials.is_empty() {
            return;
        }
        let mut cleaned = self.settings.clone();
        cleaned
            .retired_credentials
            .retain(|key| self.credentials.remove(key).is_err());
        if self.persist(&cleaned).is_ok() {
            self.settings = cleaned;
        }
    }
}

#[cfg(test)]
mod tests;
