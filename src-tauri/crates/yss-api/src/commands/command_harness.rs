use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tauri::State;
use tauri::ipc::Channel;
use yss_application::execution::ApplicationState;
use yss_automation_contract::{
    AgentDriverConfigurationFailure, AgentDriverConfigurationPort, HarnessEventEnvelope,
    HarnessEventSinkPort, HarnessSessionId, HarnessTurnId, MemoryRecordId, PersistenceFailure,
    PersistenceFuture, PrincipalId, ProjectSessionBinding, SecretCredential, WorkflowRunId,
};
use yss_statistical_harness::{HarnessError, HarnessHost, dataset_quality_review_workflow};

use crate::error::CommandError;
use crate::schema::{
    ConfigureHarnessProviderRequestDto, HarnessEventDto, HarnessMemoryRecordDto,
    HarnessRuntimeStatusDto, HarnessSessionDto, HarnessSubscriptionDto, HarnessTurnResultDto,
    WorkflowRunDto,
};

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

#[derive(Default)]
pub struct HarnessChannelHub {
    subscriptions: Mutex<BTreeMap<String, HarnessSubscription>>,
}

struct HarnessSubscription {
    session_id: HarnessSessionId,
    channel: Channel<HarnessEventDto>,
}

impl HarnessChannelHub {
    pub fn new() -> Self {
        Self::default()
    }

    fn subscribe(&self, session_id: HarnessSessionId, channel: Channel<HarnessEventDto>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.subscriptions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                id.clone(),
                HarnessSubscription {
                    session_id,
                    channel,
                },
            );
        id
    }

    fn unsubscribe(&self, subscription_id: &str) -> bool {
        self.subscriptions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(subscription_id)
            .is_some()
    }
}

impl HarnessEventSinkPort for HarnessChannelHub {
    fn publish<'a>(
        &'a self,
        event: &'a HarnessEventEnvelope,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let dto = HarnessEventDto::from(event);
            self.subscriptions
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .retain(|_, subscription| {
                    subscription.session_id != event.session_id
                        || subscription.channel.send(dto.clone()).is_ok()
                });
            Ok(())
        })
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
    let captured = application
        .capture_session()
        .map_err(|_| CommandError::expected("project_session_unavailable"))?;
    let binding = ProjectSessionBinding::new(
        captured.project_instance_id().clone(),
        captured.project_session_id().clone(),
    );
    runtime
        .host
        .reconcile_project_session(&binding)
        .await
        .map_err(map_harness_error)?;
    runtime
        .host
        .create_session(
            PrincipalId::try_new("local-user").map_err(|_| CommandError::internal("principal"))?,
            binding,
        )
        .await
        .map(HarnessSessionDto::from)
        .map_err(map_harness_error)
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
        .subscribe(session_id.clone(), on_event.clone());
    let replay = match runtime.host.events_after(&session_id, after_sequence).await {
        Ok(replay) => replay,
        Err(error) => {
            runtime.channels.unsubscribe(&subscription_id);
            return Err(map_harness_error(error));
        }
    };
    for event in replay {
        if on_event.send(HarnessEventDto::from(&event)).is_err() {
            runtime.channels.unsubscribe(&subscription_id);
            return Err(CommandError::expected("harness_channel_closed"));
        }
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
) -> Result<HarnessTurnResultDto, CommandError> {
    if !runtime.provider.is_configured() {
        return Err(CommandError::expected("assistant_provider_unavailable"));
    }
    runtime
        .host
        .submit_turn(&parse_session_id(session_id)?, message)
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
        HarnessError::Agent(
            yss_automation_contract::AgentDriverFailureCode::ProviderUnavailable,
        ) => CommandError::expected("assistant_provider_unavailable"),
        HarnessError::Agent(_) => CommandError::expected("assistant_turn_failed"),
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
