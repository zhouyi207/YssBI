use std::sync::Arc;
use yss_ipc_channel::HarnessChannelHub;

use crate::harness::HarnessSessionError;
use crate::session::ApplicationState;
use tauri::State;
use tauri::ipc::Channel;
use yss_harness_contract::{HarnessSessionId, LanguageModelSelection, PrincipalId};
use yss_harness_core::{HarnessError, HarnessHost};

use crate::ipc::error::CommandError;
use yss_ipc_contract::harness::HarnessEventDto;
use yss_ipc_contract::harness::HarnessSessionDto;
use yss_ipc_contract::harness::HarnessSubscriptionDto;
use yss_ipc_contract::harness::HarnessTurnResultDto;

mod gateway;
mod knowledge;
mod models;
pub use gateway::ApplicationCapabilityGateway;
pub use knowledge::*;
pub use models::*;

#[tauri::command]
pub async fn rename_harness_session(
    application: State<'_, ApplicationState>,
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    title: String,
) -> Result<HarnessSessionDto, CommandError> {
    let principal =
        PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?;
    application
        .rename_harness_session(
            &runtime.host,
            &principal,
            &parse_session_id(session_id)?,
            title,
        )
        .await
        .map(HarnessSessionDto::from)
        .map_err(map_session_error)
}

#[tauri::command]
pub async fn inspect_harness_tool(
    application: State<'_, ApplicationState>,
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    invocation_id: String,
) -> Result<yss_ipc_contract::harness::HarnessToolInspectionDto, CommandError> {
    let session_id = parse_session_id(session_id)?;
    let principal =
        PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?;
    application
        .validate_harness_session(&runtime.host, &principal, &session_id)
        .await
        .map_err(map_session_error)?;
    let invocation_id = yss_harness_contract::ToolInvocationId::try_new(invocation_id)
        .map_err(|_| CommandError::expected("invalid_harness_request"))?;
    let record = runtime
        .host
        .inspect_tool_invocation(&session_id, &invocation_id)
        .await
        .map_err(map_harness_error)?
        .ok_or_else(|| CommandError::expected("harness_tool_unavailable"))?;
    application
        .validate_harness_session(&runtime.host, &principal, &session_id)
        .await
        .map_err(map_session_error)?;
    Ok(record.into())
}

#[tauri::command]
pub async fn inspect_harness_citation(
    application: State<'_, ApplicationState>,
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    citation: yss_harness_contract::KnowledgeCitation,
) -> Result<yss_ipc_contract::harness::HarnessCitationDetailDto, CommandError> {
    let session_id = parse_session_id(session_id)?;
    let principal =
        PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?;
    application
        .validate_harness_session(&runtime.host, &principal, &session_id)
        .await
        .map_err(map_session_error)?;
    let body = runtime
        .host
        .inspect_citation(&session_id, &citation)
        .await
        .map_err(map_harness_error)?
        .ok_or_else(|| CommandError::expected("harness_citation_unavailable"))?;
    let resource = runtime
        .knowledge
        .citation_resource(&citation)
        .await
        .map_err(knowledge::map_knowledge_error)?;
    application
        .validate_harness_session(&runtime.host, &principal, &session_id)
        .await
        .map_err(map_session_error)?;
    Ok(yss_ipc_contract::harness::HarnessCitationDetailDto {
        text: body,
        resource,
    })
}

pub struct HarnessRuntimeState {
    host: Arc<HarnessHost>,
    channels: Arc<HarnessChannelHub>,
    models: Arc<crate::harness::models::LanguageModelService>,
    knowledge: Arc<crate::harness::knowledge::ProjectKnowledgeService>,
}

impl HarnessRuntimeState {
    pub(super) fn host(&self) -> &HarnessHost {
        &self.host
    }

    pub fn new(
        host: Arc<HarnessHost>,
        channels: Arc<HarnessChannelHub>,
        models: Arc<crate::harness::models::LanguageModelService>,
        knowledge: Arc<crate::harness::knowledge::ProjectKnowledgeService>,
    ) -> Self {
        Self {
            host,
            channels,
            models,
            knowledge,
        }
    }
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

pub(super) fn map_session_error(error: HarnessSessionError) -> CommandError {
    match error {
        HarnessSessionError::SessionCapture(_) | HarnessSessionError::ProjectUnavailable => {
            CommandError::expected("project_session_unavailable")
        }
        HarnessSessionError::Changed => CommandError::expected("project_session_changed"),
        HarnessSessionError::Host(error) => map_harness_error(error),
    }
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
    resources: Vec<yss_harness_contract::ProjectResourceRef>,
    model: Option<LanguageModelSelection>,
) -> Result<HarnessTurnResultDto, CommandError> {
    let session_id = parse_session_id(session_id)?;
    let principal =
        PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?;
    let session = application
        .open_harness_session(&runtime.host, &principal, &session_id)
        .await
        .map_err(map_session_error)?;
    runtime
        .host
        .submit_turn(&session_id, &session.project, message, resources, model)
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
        | HarnessError::Knowledge(_)) => {
            CommandError::diagnosed("harness_persistence_failed", error)
        }
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
                ProviderOutputTruncated => "assistant_provider_output_truncated",
                ProviderStreamInterrupted => "assistant_provider_stream_interrupted",
                ProviderContentFiltered => "assistant_provider_content_filtered",
                ProviderPaymentRequired => "assistant_provider_payment_required",
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
        HarnessError::Approval(_) => CommandError::expected("harness_approval_failed"),
        HarnessError::Capability(failure) => {
            use yss_harness_contract::CapabilityFailureCode;
            CommandError::expected(match failure.code {
                CapabilityFailureCode::ResourceUnavailable => "assistant_resource_unavailable",
                CapabilityFailureCode::Cancelled => "harness_turn_cancelled",
                CapabilityFailureCode::ProjectSessionChanged => "harness_session_not_active",
                _ => "harness_capability_failed",
            })
        }
    }
}
