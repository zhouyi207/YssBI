use std::sync::Arc;
use yss_ipc_channel::HarnessChannelHub;

use crate::harness::HarnessSessionError;
use crate::session::ApplicationState;
use tauri::State;
use tauri::ipc::Channel;
use yss_harness_contract::{
    AgentDriverConfigurationFailure, AgentDriverConfigurationPort, HarnessSessionId,
    MemoryRecordId, PrincipalId, SecretCredential,
};
use yss_harness_core::{HarnessError, HarnessHost};

use crate::ipc::error::CommandError;
use yss_ipc_contract::harness::ConfigureHarnessProviderRequestDto;
use yss_ipc_contract::harness::HarnessEventDto;
use yss_ipc_contract::harness::HarnessMemoryRecordDto;
use yss_ipc_contract::harness::HarnessRuntimeStatusDto;
use yss_ipc_contract::harness::HarnessSessionDto;
use yss_ipc_contract::harness::HarnessSubscriptionDto;
use yss_ipc_contract::harness::HarnessTurnResultDto;

mod gateway;
pub use gateway::ApplicationCapabilityGateway;

pub struct HarnessRuntimeState {
    host: Arc<HarnessHost>,
    channels: Arc<HarnessChannelHub>,
    provider: Arc<dyn AgentDriverConfigurationPort>,
}

impl HarnessRuntimeState {
    pub fn new(
        host: Arc<HarnessHost>,
        channels: Arc<HarnessChannelHub>,
        provider: Arc<dyn AgentDriverConfigurationPort>,
    ) -> Self {
        Self {
            host,
            channels,
            provider,
        }
    }
}

#[tauri::command]
pub fn configure_harness_provider(
    runtime: State<'_, HarnessRuntimeState>,
    request: ConfigureHarnessProviderRequestDto,
) -> Result<HarnessRuntimeStatusDto, CommandError> {
    let credential = if request.api_key.trim().is_empty() {
        None
    } else {
        Some(
            SecretCredential::new(request.api_key)
                .map_err(|_| CommandError::expected("assistant_provider_configuration_invalid"))?,
        )
    };
    let provider_configured = runtime
        .provider
        .configure(request.base_url, request.model, credential)
        .map_err(map_provider_configuration_error)?;
    Ok(HarnessRuntimeStatusDto {
        provider_configured,
    })
}

#[tauri::command]
pub async fn create_harness_session(
    application: State<'_, ApplicationState>,
    runtime: State<'_, HarnessRuntimeState>,
) -> Result<HarnessSessionDto, CommandError> {
    application
        .create_harness_session(
            &runtime.host,
            PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?,
        )
        .await
        .map(HarnessSessionDto::from)
        .map_err(map_session_error)
}

fn map_session_error(error: HarnessSessionError) -> CommandError {
    match error {
        HarnessSessionError::SessionCapture(_) | HarnessSessionError::ProjectUnavailable => {
            CommandError::expected("project_session_unavailable")
        }
        HarnessSessionError::Changed => CommandError::expected("project_session_changed"),
        HarnessSessionError::Host(error) => map_harness_error(error),
    }
}

#[tauri::command]
pub async fn list_harness_sessions(
    application: State<'_, ApplicationState>,
    runtime: State<'_, HarnessRuntimeState>,
) -> Result<Vec<HarnessSessionDto>, CommandError> {
    let principal =
        PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?;
    application
        .list_harness_sessions(&runtime.host, &principal)
        .await
        .map(|sessions| sessions.into_iter().map(HarnessSessionDto::from).collect())
        .map_err(map_session_error)
}

#[tauri::command]
pub async fn open_harness_session(
    application: State<'_, ApplicationState>,
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
) -> Result<HarnessSessionDto, CommandError> {
    let principal =
        PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?;
    application
        .open_harness_session(&runtime.host, &principal, &parse_session_id(session_id)?)
        .await
        .map(HarnessSessionDto::from)
        .map_err(map_session_error)
}

#[tauri::command]
pub async fn subscribe_harness_events(
    application: State<'_, ApplicationState>,
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    after_sequence: u64,
    on_event: Channel<HarnessEventDto>,
) -> Result<HarnessSubscriptionDto, CommandError> {
    let session_id = parse_session_id(session_id)?;
    let principal =
        PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?;
    application
        .validate_harness_session(&runtime.host, &principal, &session_id)
        .await
        .map_err(map_session_error)?;
    let subscription_id = runtime
        .channels
        .subscribe(session_id.clone(), on_event, after_sequence);
    let replay = match runtime.host.events_after(&session_id, after_sequence).await {
        Ok(replay) => replay,
        Err(error) => {
            runtime.channels.unsubscribe(&subscription_id);
            return Err(map_harness_error(error));
        }
    };
    if !runtime.channels.complete_replay(&subscription_id, replay) {
        return Err(CommandError::expected("harness_channel_closed"));
    }
    Ok(HarnessSubscriptionDto { subscription_id })
}

#[tauri::command]
pub fn unsubscribe_harness_events(
    runtime: State<'_, HarnessRuntimeState>,
    subscription_id: String,
) -> Result<(), CommandError> {
    if runtime.channels.unsubscribe(&subscription_id) {
        Ok(())
    } else {
        Err(CommandError::expected("harness_subscription_not_found"))
    }
}

#[tauri::command]
pub async fn submit_harness_turn(
    application: State<'_, ApplicationState>,
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    message: String,
    active_graph_path: Option<String>,
) -> Result<HarnessTurnResultDto, CommandError> {
    if !runtime.provider.is_configured() {
        return Err(CommandError::expected("assistant_provider_unavailable"));
    }
    let session_id = parse_session_id(session_id)?;
    let principal =
        PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?;
    application
        .open_harness_session(&runtime.host, &principal, &session_id)
        .await
        .map_err(map_session_error)?;
    runtime
        .host
        .submit_turn(&session_id, message, active_graph_path)
        .await
        .map(|result| HarnessTurnResultDto {
            final_text: result.final_text,
        })
        .map_err(map_harness_error)
}

#[tauri::command]
pub fn cancel_harness_turn(
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
) -> Result<(), CommandError> {
    if runtime.host.cancel_turn(&parse_session_id(session_id)?) {
        Ok(())
    } else {
        Err(CommandError::expected("harness_turn_not_running"))
    }
}

#[tauri::command]
pub async fn list_harness_memory(
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
) -> Result<Vec<HarnessMemoryRecordDto>, CommandError> {
    runtime
        .host
        .session_memory(&parse_session_id(session_id)?)
        .await
        .map(|records| {
            records
                .into_iter()
                .map(HarnessMemoryRecordDto::from)
                .collect()
        })
        .map_err(map_harness_error)
}

#[tauri::command]
pub async fn delete_harness_memory(
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    record_id: String,
) -> Result<(), CommandError> {
    let record_id = MemoryRecordId::try_new(record_id)
        .map_err(|_| CommandError::expected("invalid_memory_record_id"))?;
    runtime
        .host
        .delete_session_memory(&parse_session_id(session_id)?, &record_id)
        .await
        .map_err(map_harness_error)
}

fn parse_session_id(value: String) -> Result<HarnessSessionId, CommandError> {
    HarnessSessionId::try_new(value)
        .map_err(|_| CommandError::expected("invalid_harness_session_id"))
}

fn map_harness_error(error: HarnessError) -> CommandError {
    match error {
        HarnessError::Identity(_) | HarnessError::InvalidMessage => {
            CommandError::expected("invalid_harness_request")
        }
        error @ (HarnessError::IdGeneration(_)
        | HarnessError::Persistence(_)
        | HarnessError::Knowledge(_)
        | HarnessError::Memory(_)) => CommandError::diagnosed("harness_persistence_failed", error),
        HarnessError::SessionNotFound => CommandError::expected("harness_session_not_found"),
        HarnessError::SessionNotActive => CommandError::expected("harness_session_not_active"),
        HarnessError::ConcurrentTurn => CommandError::expected("harness_turn_already_running"),
        HarnessError::Agent(code) => {
            use yss_harness_contract::AgentDriverFailureCode::*;
            CommandError::expected(match code {
                ProviderUnavailable => "assistant_provider_unavailable",
                ProviderAuthenticationFailed => "assistant_authentication_failed",
                ProviderRateLimited => "assistant_rate_limited",
                ProviderRequestRejected => "assistant_provider_request_rejected",
                ContextWindowExceeded => "assistant_context_window_exceeded",
                ModelTurnLimitExceeded => "assistant_model_turn_limit_exceeded",
                ProviderTransportFailed => "assistant_provider_connection_failed",
                DeadlineElapsed => "assistant_turn_timed_out",
                InvalidProviderResponse => "assistant_invalid_provider_response",
                Cancelled => "harness_turn_cancelled",
                OutputUnavailable | InternalFailure => "assistant_turn_failed",
            })
        }
        HarnessError::Cancelled => CommandError::expected("harness_turn_cancelled"),
        HarnessError::ConcurrentWorkflow => CommandError::expected("workflow_already_running"),
        HarnessError::WorkflowCompile(_) => CommandError::expected("invalid_workflow_request"),
        HarnessError::WorkflowRuntime(_) => CommandError::expected("workflow_transition_failed"),
        HarnessError::WorkflowNotFound => CommandError::expected("workflow_run_not_found"),
        error @ HarnessError::WorkflowDefinitionNotFound => {
            CommandError::diagnosed("workflow_definition_unavailable", error)
        }
        HarnessError::WorkflowTurnNotFound | HarnessError::WorkflowTurnMismatch => {
            CommandError::expected("workflow_turn_unavailable")
        }
        HarnessError::WorkflowProjectChanged => CommandError::expected("workflow_project_changed"),
        HarnessError::WorkflowWaiting => CommandError::expected("workflow_waiting"),
        HarnessError::MemoryNotFound => CommandError::expected("harness_memory_not_found"),
        HarnessError::Approval(_) => CommandError::expected("harness_approval_failed"),
        HarnessError::Capability(_) => CommandError::expected("harness_capability_failed"),
    }
}

fn map_provider_configuration_error(error: AgentDriverConfigurationFailure) -> CommandError {
    match error {
        AgentDriverConfigurationFailure::Invalid => {
            CommandError::expected("assistant_provider_configuration_invalid")
        }
    }
}
