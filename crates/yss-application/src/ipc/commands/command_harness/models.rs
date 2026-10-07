use super::*;
use crate::harness::models::{CredentialChange, ModelSettingsError};
use yss_harness_contract::{
    LanguageModelCatalog, LanguageModelConfig, LanguageModelResolverPort, LanguageModelSelection,
    SecretCredential,
};
use yss_ipc_contract::harness::{DiscoverHarnessModelsRequestDto, SaveHarnessProviderRequestDto};

#[tauri::command]
pub async fn list_harness_models(
    runtime: State<'_, HarnessRuntimeState>,
) -> Result<LanguageModelCatalog, CommandError> {
    runtime.models.catalog().await.map_err(map_model_error)
}

#[tauri::command]
pub async fn save_harness_provider(
    runtime: State<'_, HarnessRuntimeState>,
    request: SaveHarnessProviderRequestDto,
) -> Result<LanguageModelCatalog, CommandError> {
    let credential = match request.api_key {
        None => CredentialChange::Keep,
        Some(value) if value.trim().is_empty() => CredentialChange::Remove,
        Some(value) => CredentialChange::Replace(
            SecretCredential::new(value)
                .map_err(|_| CommandError::expected("assistant_provider_configuration_invalid"))?,
        ),
    };
    runtime
        .models
        .save_provider(request.config, credential)
        .await
        .map_err(map_model_error)
}

#[tauri::command]
pub async fn delete_harness_provider(
    runtime: State<'_, HarnessRuntimeState>,
    provider_id: String,
) -> Result<LanguageModelCatalog, CommandError> {
    runtime
        .models
        .delete_provider(provider_id)
        .await
        .map_err(map_model_error)
}

#[tauri::command]
pub async fn discover_harness_models(
    runtime: State<'_, HarnessRuntimeState>,
    request: DiscoverHarnessModelsRequestDto,
) -> Result<Vec<LanguageModelConfig>, CommandError> {
    let credential = request
        .api_key
        .map(SecretCredential::new)
        .transpose()
        .map_err(|_| CommandError::expected("assistant_provider_configuration_invalid"))?;
    runtime
        .models
        .discover_models(request.config, credential)
        .await
        .map_err(map_model_error)
}

#[tauri::command]
pub async fn set_default_harness_model(
    runtime: State<'_, HarnessRuntimeState>,
    model: LanguageModelSelection,
) -> Result<LanguageModelCatalog, CommandError> {
    runtime
        .models
        .set_default(model)
        .await
        .map_err(map_model_error)
}

#[tauri::command]
pub async fn select_harness_model(
    application: State<'_, ApplicationState>,
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    model: LanguageModelSelection,
) -> Result<HarnessSessionDto, CommandError> {
    runtime
        .models
        .resolve(Some(&model))
        .await
        .map_err(|error| map_harness_error(HarnessError::Agent(error.code)))?;
    let principal =
        PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?;
    application
        .select_harness_model(
            &runtime.host,
            &principal,
            &parse_session_id(session_id)?,
            model,
        )
        .await
        .map(HarnessSessionDto::from)
        .map_err(map_session_error)
}

fn map_model_error(error: ModelSettingsError) -> CommandError {
    match error {
        ModelSettingsError::Invalid => {
            CommandError::expected("assistant_provider_configuration_invalid")
        }
        ModelSettingsError::Unavailable => {
            CommandError::expected("assistant_model_settings_unavailable")
        }
        ModelSettingsError::Credentials => {
            CommandError::expected("assistant_credentials_unavailable")
        }
        ModelSettingsError::ModelUnavailable => {
            CommandError::expected("assistant_provider_unavailable")
        }
        ModelSettingsError::Provider(code) => map_harness_error(HarnessError::Agent(code)),
    }
}
