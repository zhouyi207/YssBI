use std::sync::Arc;
use yss_ipc_channel::{HarnessChannelHub, HarnessGraphClientHub};

use tauri::State;
use tauri::ipc::Channel;
use yss_application::execution::ApplicationState;
use yss_application::harness::HarnessSessionError;
use yss_automation_contract::{
    AgentDriverConfigurationFailure, AgentDriverConfigurationPort, HarnessSessionId, HarnessTurnId,
    MemoryRecordId, PrincipalId, SecretCredential, WorkflowRunId,
};
use yss_statistical_harness::{HarnessError, HarnessHost, dataset_quality_review_workflow};

use crate::error::CommandError;
use yss_ipc_contract::harness::ConfigureHarnessProviderRequestDto;
use yss_ipc_contract::harness::HarnessEventDto;
use yss_ipc_contract::harness::HarnessMemoryRecordDto;
use yss_ipc_contract::harness::HarnessRuntimeStatusDto;
use yss_ipc_contract::harness::HarnessSessionDto;
use yss_ipc_contract::harness::HarnessSubscriptionDto;
use yss_ipc_contract::harness::HarnessTurnResultDto;
use yss_ipc_contract::harness::WorkflowRunDto;

mod gateway;
pub use gateway::ApplicationCapabilityGateway;
mod graph_client;
pub use graph_client::*;

pub struct HarnessRuntimeState {
    host: Arc<HarnessHost>,
    channels: Arc<HarnessChannelHub>,
    provider: Arc<dyn AgentDriverConfigurationPort>,
    graph_clients: Arc<HarnessGraphClientHub>,
}

impl HarnessRuntimeState {
    pub fn new(
        host: Arc<HarnessHost>,
        channels: Arc<HarnessChannelHub>,
        provider: Arc<dyn AgentDriverConfigurationPort>,
        graph_clients: Arc<HarnessGraphClientHub>,
    ) -> Self {
        Self {
            host,
            channels,
            provider,
            graph_clients,
        }
    }
}

#[tauri::command]
pub fn get_harness_runtime_status(
    runtime: State<'_, HarnessRuntimeState>,
) -> HarnessRuntimeStatusDto {
    HarnessRuntimeStatusDto {
        provider_configured: runtime.provider.is_configured(),
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
        .map_err(|error| match error {
            HarnessSessionError::SessionCapture(_) => {
                CommandError::expected("project_session_unavailable")
            }
            HarnessSessionError::Host(error) => map_harness_error(error),
        })
}

#[tauri::command]
pub async fn subscribe_harness_events(
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    after_sequence: u64,
    on_event: Channel<HarnessEventDto>,
) -> Result<HarnessSubscriptionDto, CommandError> {
    let session_id = parse_session_id(session_id)?;
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
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    message: String,
    active_graph_path: Option<String>,
) -> Result<HarnessTurnResultDto, CommandError> {
    if !runtime.provider.is_configured() {
        return Err(CommandError::expected("assistant_provider_unavailable"));
    }
    runtime
        .host
        .submit_turn(&parse_session_id(session_id)?, message, active_graph_path)
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
pub async fn close_harness_session(
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
) -> Result<(), CommandError> {
    runtime
        .host
        .close_session(&parse_session_id(session_id)?)
        .await
        .map_err(map_harness_error)
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

#[tauri::command]
pub async fn plan_dataset_quality_review(
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    turn_id: String,
    database_id: String,
) -> Result<WorkflowRunDto, CommandError> {
    let session_id = parse_session_id(session_id)?;
    let turn_id = HarnessTurnId::try_new(turn_id)
        .map_err(|_| CommandError::expected("invalid_harness_turn_id"))?;
    let workflow = dataset_quality_review_workflow(database_id)
        .map_err(|_| CommandError::expected("invalid_workflow_request"))?;
    runtime
        .host
        .plan_workflow(&session_id, Some(&turn_id), &workflow)
        .await
        .map(WorkflowRunDto::from)
        .map_err(map_harness_error)
}

#[tauri::command]
pub async fn advance_harness_workflow(
    runtime: State<'_, HarnessRuntimeState>,
    run_id: String,
) -> Result<WorkflowRunDto, CommandError> {
    let run_id = WorkflowRunId::try_new(run_id)
        .map_err(|_| CommandError::expected("invalid_workflow_run_id"))?;
    runtime
        .host
        .advance_workflow(&run_id)
        .await
        .map(WorkflowRunDto::from)
        .map_err(map_harness_error)
}

#[tauri::command]
pub async fn pause_harness_workflow(
    runtime: State<'_, HarnessRuntimeState>,
    run_id: String,
) -> Result<WorkflowRunDto, CommandError> {
    let run_id = parse_workflow_run_id(run_id)?;
    runtime
        .host
        .pause_workflow(&run_id)
        .await
        .map(WorkflowRunDto::from)
        .map_err(map_harness_error)
}

#[tauri::command]
pub async fn resume_harness_workflow(
    runtime: State<'_, HarnessRuntimeState>,
    run_id: String,
) -> Result<WorkflowRunDto, CommandError> {
    let run_id = parse_workflow_run_id(run_id)?;
    runtime
        .host
        .resume_workflow(&run_id)
        .await
        .map(WorkflowRunDto::from)
        .map_err(map_harness_error)
}

#[tauri::command]
pub async fn cancel_harness_workflow(
    runtime: State<'_, HarnessRuntimeState>,
    run_id: String,
) -> Result<WorkflowRunDto, CommandError> {
    let run_id = parse_workflow_run_id(run_id)?;
    runtime
        .host
        .cancel_workflow(&run_id)
        .await
        .map(WorkflowRunDto::from)
        .map_err(map_harness_error)
}

fn parse_session_id(value: String) -> Result<HarnessSessionId, CommandError> {
    HarnessSessionId::try_new(value)
        .map_err(|_| CommandError::expected("invalid_harness_session_id"))
}

fn parse_workflow_run_id(value: String) -> Result<WorkflowRunId, CommandError> {
    WorkflowRunId::try_new(value).map_err(|_| CommandError::expected("invalid_workflow_run_id"))
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
            use yss_automation_contract::AgentDriverFailureCode::*;
            CommandError::expected(match code {
                ProviderUnavailable => "assistant_provider_unavailable",
                ProviderAuthenticationFailed => "assistant_authentication_failed",
                ProviderRateLimited => "assistant_rate_limited",
                ProviderRequestRejected => "assistant_provider_request_rejected",
                ProviderTransportFailed => "assistant_provider_connection_failed",
                DeadlineElapsed => "assistant_turn_timed_out",
                InvalidProviderResponse => "assistant_invalid_provider_response",
                Cancelled => "harness_turn_cancelled",
                OutputUnavailable | InternalFailure => "assistant_turn_failed",
            })
        }
        HarnessError::Cancelled => CommandError::expected("harness_turn_cancelled"),
        HarnessError::TurnStillRunning => CommandError::expected("harness_turn_still_running"),
        error @ HarnessError::SequenceExhausted => {
            CommandError::diagnosed("harness_sequence_exhausted", error)
        }
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
