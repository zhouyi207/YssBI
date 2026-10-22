//! Application-owned web search settings. The OS owns the search API keys.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use yss_harness_contract::{SecretCredential, WebAccessPort};
use yss_harness_web::{SearchBackend, WebAccess};

use super::models::{CredentialChange, ModelCredentials, ModelSettingsError};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchProvider {
    #[default]
    Off,
    Brave,
    Tavily,
    Searxng,
}

impl WebSearchProvider {
    pub const ALL: [Self; 4] = [Self::Off, Self::Brave, Self::Tavily, Self::Searxng];

    pub fn requires_api_key(self) -> bool {
        matches!(self, Self::Brave | Self::Tavily)
    }

    fn credential_key(self) -> Option<&'static str> {
        // Shares the system credential service with model providers; the prefix keeps
        // these entries apart from provider credential keys.
        match self {
            Self::Brave => Some("web-search:brave"),
            Self::Tavily => Some("web-search:tavily"),
            Self::Off | Self::Searxng => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebSearchConfig {
    provider: WebSearchProvider,
    searxng_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSearchStatus {
    pub provider: WebSearchProvider,
    pub searxng_url: String,
    pub has_api_key: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum WebSearchSettingsError {
    #[error("the search API key is required")]
    MissingApiKey,
    #[error("the SearXNG address must be an http or https URL")]
    InvalidAddress,
    #[error("web search settings are unavailable")]
    Unavailable,
    #[error("system credential store is unavailable")]
    Credentials,
}

impl From<ModelSettingsError> for WebSearchSettingsError {
    fn from(error: ModelSettingsError) -> Self {
        match error {
            ModelSettingsError::Credentials => Self::Credentials,
            _ => Self::Unavailable,
        }
    }
}

struct State {
    path: PathBuf,
    config: WebSearchConfig,
    credentials: Arc<dyn ModelCredentials>,
}

pub struct WebSearchService {
    state: Arc<Mutex<State>>,
    access: Arc<WebAccess>,
}

impl WebSearchService {
    /// Reads the saved configuration; an unreadable configuration starts with search off.
    pub fn new(
        app_dir: PathBuf,
        credentials: Arc<dyn ModelCredentials>,
    ) -> Result<Self, WebSearchSettingsError> {
        let path = app_dir.join("settings").join("web-search.json");
        let config = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                tracing::warn!(%error, "Web search settings are unreadable; search is off");
                WebSearchConfig::default()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                WebSearchConfig::default()
            }
            Err(error) => {
                tracing::warn!(%error, "Web search settings are unreadable; search is off");
                WebSearchConfig::default()
            }
        };
        let access = Arc::new(WebAccess::new(None).map_err(|error| {
            tracing::error!(%error, "Web client could not be created");
            WebSearchSettingsError::Unavailable
        })?);
        let state = State {
            path,
            config,
            credentials,
        };
        match state.backend() {
            Ok(backend) => access.set_backend(backend),
            Err(error) => tracing::warn!(%error, "Web search backend is unavailable"),
        }
        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            access,
        })
    }

    pub fn port(&self) -> Arc<dyn WebAccessPort> {
        self.access.clone()
    }

    pub async fn status(&self) -> Result<WebSearchStatus, WebSearchSettingsError> {
        self.access_state(|state| {
            Ok(WebSearchStatus {
                provider: state.config.provider,
                searxng_url: state.config.searxng_url.clone(),
                has_api_key: state.api_key(state.config.provider)?.is_some(),
            })
        })
        .await
    }

    pub async fn save(
        &self,
        provider: WebSearchProvider,
        searxng_url: String,
        api_key: CredentialChange,
    ) -> Result<WebSearchStatus, WebSearchSettingsError> {
        let access = self.access.clone();
        self.access_state(move |state| {
            let searxng_url = searxng_url.trim().to_owned();
            if provider == WebSearchProvider::Searxng && searxng_base(&searxng_url).is_none() {
                return Err(WebSearchSettingsError::InvalidAddress);
            }
            if let Some(key) = provider.credential_key() {
                match api_key {
                    CredentialChange::Keep => {}
                    CredentialChange::Replace(credential) => {
                        state.credentials.write(key, &credential)?
                    }
                    CredentialChange::Remove => state.credentials.remove(key)?,
                }
            }
            let config = WebSearchConfig {
                provider,
                searxng_url,
            };
            let backend = State {
                path: state.path.clone(),
                config: config.clone(),
                credentials: state.credentials.clone(),
            }
            .backend()?;
            state.persist(&config)?;
            state.config = config;
            access.set_backend(backend);
            Ok(WebSearchStatus {
                provider,
                searxng_url: state.config.searxng_url.clone(),
                has_api_key: state.api_key(provider)?.is_some(),
            })
        })
        .await
    }

    async fn access_state<T: Send + 'static>(
        &self,
        action: impl FnOnce(&mut State) -> Result<T, WebSearchSettingsError> + Send + 'static,
    ) -> Result<T, WebSearchSettingsError> {
        let state = self.state.clone();
        tokio::task::spawn_blocking(move || {
            let mut state = state
                .lock()
                .map_err(|_| WebSearchSettingsError::Unavailable)?;
            action(&mut state)
        })
        .await
        .map_err(|_| WebSearchSettingsError::Unavailable)?
    }
}

impl State {
    fn api_key(
        &self,
        provider: WebSearchProvider,
    ) -> Result<Option<SecretCredential>, WebSearchSettingsError> {
        match provider.credential_key() {
            Some(key) => Ok(self.credentials.read(key)?),
            None => Ok(None),
        }
    }

    fn backend(&self) -> Result<Option<SearchBackend>, WebSearchSettingsError> {
        let provider = self.config.provider;
        let api_key = || -> Result<String, WebSearchSettingsError> {
            Ok(self
                .api_key(provider)?
                .ok_or(WebSearchSettingsError::MissingApiKey)?
                .expose()
                .to_owned())
        };
        Ok(match provider {
            WebSearchProvider::Off => None,
            WebSearchProvider::Brave => Some(SearchBackend::Brave {
                api_key: api_key()?,
            }),
            WebSearchProvider::Tavily => Some(SearchBackend::Tavily {
                api_key: api_key()?,
            }),
            WebSearchProvider::Searxng => Some(SearchBackend::Searxng {
                base_url: searxng_base(&self.config.searxng_url)
                    .ok_or(WebSearchSettingsError::InvalidAddress)?,
            }),
        })
    }

    fn persist(&self, config: &WebSearchConfig) -> Result<(), WebSearchSettingsError> {
        std::fs::create_dir_all(
            self.path
                .parent()
                .ok_or(WebSearchSettingsError::Unavailable)?,
        )
        .map_err(|_| WebSearchSettingsError::Unavailable)?;
        let bytes =
            serde_json::to_vec_pretty(config).map_err(|_| WebSearchSettingsError::Unavailable)?;
        atomicwrites::AtomicFile::new(&self.path, atomicwrites::AllowOverwrite)
            .write(|file| {
                file.write_all(&bytes)?;
                file.sync_all()
            })
            .map_err(|_| WebSearchSettingsError::Unavailable)
    }
}

/// The instance root with a trailing slash, so `search` joins below any path prefix.
fn searxng_base(address: &str) -> Option<url::Url> {
    let mut url = url::Url::parse(address).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
        return None;
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Credentials(Mutex<BTreeMap<String, String>>);

    impl ModelCredentials for Credentials {
        fn read(&self, key: &str) -> Result<Option<SecretCredential>, ModelSettingsError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .get(key)
                .map(|value| SecretCredential::new(value.clone()).unwrap()))
        }

        fn write(
            &self,
            key: &str,
            credential: &SecretCredential,
        ) -> Result<(), ModelSettingsError> {
            self.0
                .lock()
                .unwrap()
                .insert(key.into(), credential.expose().into());
            Ok(())
        }

        fn remove(&self, key: &str) -> Result<(), ModelSettingsError> {
            self.0.lock().unwrap().remove(key);
            Ok(())
        }
    }

    #[tokio::test]
    async fn settings_require_their_secret_or_address_and_survive_restart() {
        let directory =
            std::env::temp_dir().join(format!("yss-web-search-{}", uuid::Uuid::new_v4()));
        let credentials = Arc::new(Credentials::default());
        let service = WebSearchService::new(directory.clone(), credentials.clone()).unwrap();
        assert!(matches!(
            service
                .save(
                    WebSearchProvider::Brave,
                    String::new(),
                    CredentialChange::Keep
                )
                .await,
            Err(WebSearchSettingsError::MissingApiKey)
        ));
        assert!(matches!(
            service
                .save(
                    WebSearchProvider::Searxng,
                    "ftp://x".into(),
                    CredentialChange::Keep
                )
                .await,
            Err(WebSearchSettingsError::InvalidAddress)
        ));
        let status = service
            .save(
                WebSearchProvider::Brave,
                String::new(),
                CredentialChange::Replace(SecretCredential::new("key").unwrap()),
            )
            .await
            .unwrap();
        assert!(status.has_api_key);
        let reopened = WebSearchService::new(directory.clone(), credentials).unwrap();
        assert_eq!(reopened.status().await.unwrap(), status);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn searxng_addresses_keep_their_path_prefix() {
        assert_eq!(
            searxng_base("https://search.example/searx?x=1")
                .unwrap()
                .join("search")
                .unwrap()
                .as_str(),
            "https://search.example/searx/search"
        );
        assert!(searxng_base("search.example").is_none());
    }
}
