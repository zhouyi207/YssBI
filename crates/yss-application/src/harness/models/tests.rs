use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
struct Credentials {
    entries: Mutex<BTreeMap<String, String>>,
    refuse_removal: AtomicBool,
}

impl ModelCredentials for Credentials {
    fn read(&self, key: &str) -> Result<Option<SecretCredential>, ModelSettingsError> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(key)
            .map(|value| SecretCredential::new(value.clone()).unwrap()))
    }
    fn write(&self, key: &str, value: &SecretCredential) -> Result<(), ModelSettingsError> {
        self.entries
            .lock()
            .unwrap()
            .insert(key.into(), value.expose().into());
        Ok(())
    }
    fn remove(&self, key: &str) -> Result<(), ModelSettingsError> {
        if self.refuse_removal.load(Ordering::Acquire) {
            return Err(ModelSettingsError::Credentials);
        }
        self.entries.lock().unwrap().remove(key);
        Ok(())
    }
}

fn provider() -> LanguageModelProviderConfig {
    LanguageModelProviderConfig {
        id: "local".into(),
        name: "Local provider".into(),
        custom_name: None,
        protocol: LanguageModelProtocol::OpenAiChat,
        adapter: "openai/openai".into(),
        authentication: LanguageModelAuthentication::ApiKey,
        base_url: "http://127.0.0.1:9/v1".into(),
        models: vec![LanguageModelConfig {
            reasoning_efforts: Vec::new(),
            id: "model-a".into(),
            name: "Model A".into(),
            context_window: None,
            max_output_tokens: None,
            temperature: None,
            top_p: None,
            additional_parameters: Default::default(),
        }],
    }
}

#[tokio::test]
async fn catalog_projects_declared_reasoning_defaults_without_guessing_or_saving_a_second_value() {
    let dir = tempfile::tempdir().unwrap();
    let credentials = Arc::new(Credentials::default());
    let service = LanguageModelService::new(dir.path().into(), credentials.clone());
    let mut config = provider();
    config.authentication = LanguageModelAuthentication::None;
    config.models[0]
        .additional_parameters
        .insert("reasoning_effort".into(), "high".into());
    let catalog = service
        .save_provider(config.clone(), CredentialChange::Keep)
        .await
        .unwrap();
    assert_eq!(
        catalog.providers[0].reasoning_defaults.get("model-a"),
        Some(&ReasoningEffort::High)
    );
    let saved: serde_json::Value = serde_json::from_slice(
        &std::fs::read(dir.path().join("settings/language-models.json")).unwrap(),
    )
    .unwrap();
    assert!(saved["providers"][0].get("reasoningDefaults").is_none());
    let reopened = LanguageModelService::new(dir.path().into(), credentials);
    assert_eq!(reopened.catalog().await.unwrap(), catalog);
    config.models[0].additional_parameters.clear();
    let catalog = service
        .save_provider(config, CredentialChange::Keep)
        .await
        .unwrap();
    assert!(catalog.providers[0].reasoning_defaults.is_empty());
}

#[tokio::test]
async fn custom_provider_names_label_runs_without_changing_account_identity() {
    let dir = tempfile::tempdir().unwrap();
    let credentials = Arc::new(Credentials::default());
    let service = LanguageModelService::new(dir.path().into(), credentials.clone());
    let mut personal = provider();
    personal.id = "personal".into();
    personal.custom_name = Some("  Personal account  ".into());
    let mut work = provider();
    work.id = "work".into();
    work.custom_name = Some("Work account".into());
    for (config, key) in [(&personal, "personal-key"), (&work, "work-key")] {
        service
            .save_provider(
                config.clone(),
                CredentialChange::Replace(SecretCredential::new(key).unwrap()),
            )
            .await
            .unwrap();
    }
    let saved_credentials = credentials.entries.lock().unwrap().clone();
    assert_eq!(saved_credentials.len(), 2);
    drop(service);

    let service = LanguageModelService::new(dir.path().into(), credentials.clone());
    let catalog = service.catalog().await.unwrap();
    assert_eq!(catalog.providers.len(), 2);
    let mut admitted = Vec::new();
    for (config, display_name) in [(&personal, "Personal account"), (&work, "Work account")] {
        let status = catalog
            .providers
            .iter()
            .find(|entry| entry.config.id == config.id)
            .unwrap();
        assert_eq!(&status.config, config);
        assert_eq!(status.config.name, "Local provider");
        assert!(status.has_api_key);
        let selection = LanguageModelSelection {
            provider_id: config.id.clone(),
            model_id: "model-a".into(),
        };
        let resolved = service.resolve(Some(&selection)).await.unwrap();
        assert_eq!(resolved.identity.selection, selection);
        assert_eq!(resolved.identity.provider_name, display_name);
        admitted.push(resolved.identity);
    }

    for custom_name in [None, Some(" \t".into())] {
        personal.custom_name = custom_name;
        let saved = service
            .save_provider(personal.clone(), CredentialChange::Keep)
            .await
            .unwrap();
        assert_eq!(
            saved
                .providers
                .iter()
                .find(|entry| entry.config.id == personal.id)
                .unwrap()
                .config,
            personal
        );
        let resolved = service.resolve(Some(&admitted[0].selection)).await.unwrap();
        assert_eq!(resolved.identity.selection, admitted[0].selection);
        assert_eq!(resolved.identity.provider_name, "Local provider");
    }
    assert_eq!(admitted[0].provider_name, "Personal account");
    assert_eq!(admitted[1].provider_name, "Work account");
    assert_eq!(*credentials.entries.lock().unwrap(), saved_credentials);
}

#[tokio::test]
async fn model_discovery_discards_results_when_its_connection_changes() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut config = provider();
    config.base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let server = {
        let entered = entered.clone();
        let release = release.clone();
        tokio::spawn(async move {
            for request_number in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut bytes = [0; 1024];
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let read = stream.read(&mut bytes).await.unwrap();
                    assert_ne!(read, 0);
                    request.extend_from_slice(&bytes[..read]);
                }
                if request_number == 1 {
                    entered.notify_one();
                    release.notified().await;
                }
                let body = r#"{"object":"list","data":[{"id":"listed-model","object":"model","created":0,"owned_by":"fixture"}]}"#;
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
        })
    };
    let dir = tempfile::tempdir().unwrap();
    let service = Arc::new(LanguageModelService::new(
        dir.path().into(),
        Arc::new(Credentials::default()),
    ));
    service
        .save_provider(
            config.clone(),
            CredentialChange::Replace(SecretCredential::new("fixture-credential").unwrap()),
        )
        .await
        .unwrap();
    let models = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        service.discover_models(config.clone(), None),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(models[0].id, "listed-model");
    let pending = {
        let service = service.clone();
        let config = config.clone();
        tokio::spawn(async move { service.discover_models(config, None).await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    config.base_url = "http://127.0.0.1:9/v1".into();
    service
        .save_provider(config, CredentialChange::Keep)
        .await
        .unwrap();
    release.notify_one();
    assert!(matches!(
        pending.await.unwrap(),
        Err(ModelSettingsError::ModelUnavailable)
    ));
    server.await.unwrap();
}

async fn discovery_server(
    listener: tokio::net::TcpListener,
    expected_keys: Vec<Option<&'static str>>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for key in expected_keys {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut bytes = [0; 1024];
        while !request.windows(4).any(|part| part == b"\r\n\r\n") {
            let read = stream.read(&mut bytes).await.unwrap();
            assert_ne!(read, 0);
            request.extend_from_slice(&bytes[..read]);
        }
        let request = String::from_utf8(request).unwrap();
        assert!(request.starts_with("GET /v1/models "));
        let authorization = request.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("authorization")
                .then(|| value.trim())
        });
        let expected = key.map(|key| format!("Bearer {key}"));
        assert_eq!(authorization, expected.as_deref());
        let body = r#"{"object":"list","data":[{"id":"listed-model","object":"model","created":0,"owned_by":"fixture"}]}"#;
        stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    }
}

#[tokio::test]
async fn model_discovery_queries_unsaved_connections_without_persisting_them() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut config = provider();
    config.base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    config.models[0].id.clear();
    let server = tokio::spawn(discovery_server(listener, vec![Some("draft-key"), None]));
    let dir = tempfile::tempdir().unwrap();
    let credentials = Arc::new(Credentials::default());
    let service = LanguageModelService::new(dir.path().into(), credentials.clone());

    assert!(matches!(
        service.discover_models(config.clone(), None).await,
        Err(ModelSettingsError::ModelUnavailable)
    ));
    let models = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        service.discover_models(
            config.clone(),
            Some(SecretCredential::new("draft-key").unwrap()),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(models[0].id, "listed-model");

    config.authentication = LanguageModelAuthentication::None;
    let models = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        service.discover_models(config, None),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(models[0].id, "listed-model");
    assert!(service.catalog().await.unwrap().providers.is_empty());
    assert!(credentials.entries.lock().unwrap().is_empty());
    assert!(!dir.path().join("settings/language-models.json").exists());
    server.await.unwrap();
}

#[tokio::test]
async fn credential_scope_changes_require_replacement_before_saving() {
    let dir = tempfile::tempdir().unwrap();
    let credentials = Arc::new(Credentials::default());
    let service = LanguageModelService::new(dir.path().into(), credentials.clone());
    let saved = service
        .save_provider(
            provider(),
            CredentialChange::Replace(SecretCredential::new("original-provider-key").unwrap()),
        )
        .await
        .unwrap();
    let path = dir.path().join("settings/language-models.json");
    let content = std::fs::read(&path).unwrap();
    let original_keys = credentials.entries.lock().unwrap().clone();

    let mut renamed = provider();
    renamed.name = "Another provider".into();
    let mut adapted = provider();
    adapted.adapter = "deepseek/openai".into();
    for changed in [renamed.clone(), adapted] {
        assert!(matches!(
            service.save_provider(changed, CredentialChange::Keep).await,
            Err(ModelSettingsError::Invalid)
        ));
        assert_eq!(service.catalog().await.unwrap(), saved);
        assert_eq!(std::fs::read(&path).unwrap(), content);
        assert_eq!(*credentials.entries.lock().unwrap(), original_keys);
    }

    let replacement = service
        .save_provider(
            renamed.clone(),
            CredentialChange::Replace(SecretCredential::new("replacement-provider-key").unwrap()),
        )
        .await
        .unwrap();
    assert_eq!(replacement.providers[0].config, renamed);
    assert!(replacement.providers[0].has_api_key);
    assert_eq!(
        credentials
            .entries
            .lock()
            .unwrap()
            .values()
            .collect::<Vec<_>>(),
        vec!["replacement-provider-key"]
    );
}

#[tokio::test]
async fn credential_scope_changes_require_replacement_before_discovery() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut changed = provider();
    changed.base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(discovery_server(
        listener,
        vec![Some("replacement-provider-key")],
    ));
    let dir = tempfile::tempdir().unwrap();
    let credentials = Arc::new(Credentials::default());
    let service = LanguageModelService::new(dir.path().into(), credentials.clone());
    let saved = service
        .save_provider(
            provider(),
            CredentialChange::Replace(SecretCredential::new("original-provider-key").unwrap()),
        )
        .await
        .unwrap();
    let path = dir.path().join("settings/language-models.json");
    let content = std::fs::read(&path).unwrap();
    let original_keys = credentials.entries.lock().unwrap().clone();

    changed.name = "Another provider".into();
    let mut adapted = changed.clone();
    adapted.name = provider().name;
    adapted.adapter = "deepseek/openai".into();
    for draft in [changed.clone(), adapted] {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            service.discover_models(draft, None),
        )
        .await
        .unwrap();
        assert!(matches!(result, Err(ModelSettingsError::Invalid)));
    }
    let models = service
        .discover_models(
            changed,
            Some(SecretCredential::new("replacement-provider-key").unwrap()),
        )
        .await
        .unwrap();
    assert_eq!(models[0].id, "listed-model");
    assert_eq!(service.catalog().await.unwrap(), saved);
    assert_eq!(std::fs::read(path).unwrap(), content);
    assert_eq!(*credentials.entries.lock().unwrap(), original_keys);
    server.await.unwrap();
}

#[tokio::test]
async fn model_discovery_uses_drafts_without_replacing_saved_connections_or_keys() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut draft = provider();
    draft.base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(discovery_server(
        listener,
        vec![Some("saved-key"), Some("temporary-key")],
    ));
    let dir = tempfile::tempdir().unwrap();
    let credentials = Arc::new(Credentials::default());
    let service = LanguageModelService::new(dir.path().into(), credentials.clone());
    let saved = service
        .save_provider(
            provider(),
            CredentialChange::Replace(SecretCredential::new("saved-key").unwrap()),
        )
        .await
        .unwrap();
    let settings_path = dir.path().join("settings/language-models.json");
    let saved_file = std::fs::read(&settings_path).unwrap();
    let saved_credentials = credentials.entries.lock().unwrap().clone();

    for credential in [None, Some(SecretCredential::new("temporary-key").unwrap())] {
        let models = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            service.discover_models(draft.clone(), credential),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(models[0].id, "listed-model");
    }
    assert_eq!(service.catalog().await.unwrap(), saved);
    assert_eq!(std::fs::read(settings_path).unwrap(), saved_file);
    assert_eq!(*credentials.entries.lock().unwrap(), saved_credentials);
    server.await.unwrap();
}

#[tokio::test]
async fn provider_settings_keep_secrets_out_of_files_and_retire_keys_durably() {
    let dir = tempfile::tempdir().unwrap();
    let credentials = Arc::new(Credentials::default());
    let service = LanguageModelService::new(dir.path().into(), credentials.clone());
    assert!(service.resolve(None).await.is_err());
    let catalog = service
        .save_provider(
            provider(),
            CredentialChange::Replace(SecretCredential::new("secret-first").unwrap()),
        )
        .await
        .unwrap();
    assert!(catalog.providers[0].has_api_key);
    let resolved = service.resolve(None).await.unwrap();
    assert_eq!(resolved.identity.selection.model_id, "model-a");
    credentials.refuse_removal.store(true, Ordering::Release);
    service
        .save_provider(
            provider(),
            CredentialChange::Replace(SecretCredential::new("secret-second").unwrap()),
        )
        .await
        .unwrap();
    let settings_path = dir.path().join("settings/language-models.json");
    let raw = std::fs::read_to_string(&settings_path).unwrap();
    assert!(!raw.contains("secret-first") && !raw.contains("secret-second"));
    assert!(
        !serde_json::to_string(&catalog)
            .unwrap()
            .contains("secret-first")
    );
    assert_eq!(credentials.entries.lock().unwrap().len(), 2);
    drop(service);
    credentials.refuse_removal.store(false, Ordering::Release);
    let service = LanguageModelService::new(dir.path().into(), credentials.clone());
    assert!(service.catalog().await.unwrap().providers[0].has_api_key);
    assert_eq!(credentials.entries.lock().unwrap().len(), 1);
    let mut updated = provider();
    updated.models[0].id = "model-b".into();
    service
        .save_provider(updated, CredentialChange::Keep)
        .await
        .unwrap();
    assert_eq!(resolved.identity.selection.model_id, "model-a");
    assert_eq!(
        service
            .resolve(None)
            .await
            .unwrap()
            .identity
            .selection
            .model_id,
        "model-b"
    );
    assert!(
        service
            .resolve(Some(&resolved.identity.selection))
            .await
            .is_err()
    );
    let cleared = service
        .save_provider(provider(), CredentialChange::Remove)
        .await
        .unwrap();
    assert!(!cleared.providers[0].has_api_key);
    assert!(credentials.entries.lock().unwrap().is_empty());
    assert!(service.resolve(None).await.is_err());
    service
        .save_provider(
            provider(),
            CredentialChange::Replace(SecretCredential::new("retired-local-key").unwrap()),
        )
        .await
        .unwrap();
    let mut local = provider();
    local.authentication = LanguageModelAuthentication::None;
    local.models[0].temperature = Some(0.3);
    local.models[0]
        .additional_parameters
        .insert("seed".into(), 7.into());
    let saved = service
        .save_provider(local.clone(), CredentialChange::Keep)
        .await
        .unwrap();
    assert!(!saved.providers[0].has_api_key);
    assert!(credentials.entries.lock().unwrap().is_empty());
    assert!(service.resolve(None).await.is_ok());
    drop(service);
    let service = LanguageModelService::new(dir.path().into(), credentials.clone());
    assert_eq!(service.catalog().await.unwrap().providers[0].config, local);
    assert!(service.resolve(None).await.is_ok());
    assert!(
        service
            .delete_provider("local".into())
            .await
            .unwrap()
            .providers
            .is_empty()
    );
}
