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

#[derive(Default)]
pub struct HarnessChannelHub {
    subscriptions: Mutex<BTreeMap<String, HarnessSubscription>>,
}

struct HarnessSubscription {
    session_id: HarnessSessionId,
    channel: Channel<HarnessEventDto>,
    last_sequence: u64,
    replaying: bool,
    pending: BTreeMap<u64, HarnessEventDto>,
}

const MAX_PENDING_HARNESS_EVENTS: usize = 256;

impl HarnessSubscription {
    fn enqueue(&mut self, event: HarnessEventDto) -> bool {
        if event.sequence > self.last_sequence {
            self.pending.insert(event.sequence, event);
        }
        if !self.flush() {
            return false;
        }
        if self.pending.len() > MAX_PENDING_HARNESS_EVENTS {
            // A durable event beyond the gap makes the client request replay. Do not keep
            // accumulating an unbounded live queue behind a missing sequence.
            if let Some((_, event)) = self.pending.pop_last() {
                let _ = self.channel.send(event);
            }
            return false;
        }
        true
    }

    fn flush(&mut self) -> bool {
        if self.replaying {
            return true;
        }
        while let Some(sequence) = self.last_sequence.checked_add(1) {
            let Some(event) = self.pending.remove(&sequence) else {
                break;
            };
            if self.channel.send(event).is_err() {
                return false;
            }
            self.last_sequence = sequence;
        }
        true
    }
}

impl HarnessChannelHub {
    pub fn new() -> Self {
        Self::default()
    }

    fn subscribe(
        &self,
        session_id: HarnessSessionId,
        channel: Channel<HarnessEventDto>,
        after_sequence: u64,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.subscriptions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                id.clone(),
                HarnessSubscription {
                    session_id,
                    channel,
                    last_sequence: after_sequence,
                    replaying: true,
                    pending: BTreeMap::new(),
                },
            );
        id
    }

    fn complete_replay(&self, subscription_id: &str, events: Vec<HarnessEventEnvelope>) -> bool {
        let mut subscriptions = self
            .subscriptions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(subscription) = subscriptions.get_mut(subscription_id) else {
            return false;
        };
        subscription.replaying = false;
        for event in events {
            if !subscription.enqueue(HarnessEventDto::from(&event)) {
                subscriptions.remove(subscription_id);
                return false;
            }
        }
        if subscription.flush() {
            true
        } else {
            subscriptions.remove(subscription_id);
            false
        }
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
                    subscription.session_id != event.session_id || subscription.enqueue(dto.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use yss_automation_contract::{HarnessEvent, UnixMillis};

    #[test]
    fn live_events_wait_for_replay_and_are_delivered_once_in_sequence() {
        let delivered = Arc::new(Mutex::new(Vec::<u64>::new()));
        let received = delivered.clone();
        let channel = Channel::new(move |body| {
            let tauri::ipc::InvokeResponseBody::Json(body) = body else {
                panic!("expected JSON")
            };
            received.lock().unwrap().push(
                serde_json::from_str::<serde_json::Value>(&body).unwrap()["sequence"]
                    .as_u64()
                    .unwrap(),
            );
            Ok(())
        });
        let session = HarnessSessionId::try_new("session-1").unwrap();
        let event = |sequence| HarnessEventEnvelope {
            sequence,
            session_id: session.clone(),
            turn_id: None,
            occurred_at: UnixMillis::from_existing(1000),
            event: HarnessEvent::SessionCreated,
        };
        let hub = HarnessChannelHub::new();
        let id = hub.subscribe(session.clone(), channel.clone(), 0);
        tauri::async_runtime::block_on(hub.publish(&event(3))).unwrap();
        assert!(delivered.lock().unwrap().is_empty());
        assert!(hub.complete_replay(&id, vec![event(1), event(2), event(3)]));
        tauri::async_runtime::block_on(hub.publish(&event(3))).unwrap();
        tauri::async_runtime::block_on(hub.publish(&event(5))).unwrap();
        tauri::async_runtime::block_on(hub.publish(&event(4))).unwrap();
        assert_eq!(*delivered.lock().unwrap(), [1, 2, 3, 4, 5]);
        for sequence in 1000..=1000 + MAX_PENDING_HARNESS_EVENTS as u64 {
            tauri::async_runtime::block_on(hub.publish(&event(sequence))).unwrap();
        }
        assert_eq!(
            delivered.lock().unwrap().last().copied(),
            Some(1000 + MAX_PENDING_HARNESS_EVENTS as u64)
        );
        assert!(!hub.unsubscribe(&id));
        delivered.lock().unwrap().clear();
        let id = hub.subscribe(session.clone(), channel, 0);
        let history = (1..=MAX_PENDING_HARNESS_EVENTS as u64 * 2)
            .map(event)
            .collect();
        assert!(hub.complete_replay(&id, history));
        assert_eq!(
            delivered.lock().unwrap().len(),
            MAX_PENDING_HARNESS_EVENTS * 2
        );
    }
}
