//! Rig adapter for the provider-neutral Harness Agent Driver port.

#![forbid(unsafe_code)]

use futures_util::StreamExt;
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use rig_agent::agent::{AgentBuilder, MultiTurnStreamItem, StreamingError, StreamingResult};
use rig_agent::completion::PromptError;
use rig_agent::streaming::{StreamedAssistantContent, StreamingPrompt};
use rig_agent::tool::{DynamicTool, ToolExecutionError, ToolOutput};
use rig_core::client::CompletionClient;
use rig_core::completion::{CompletionError, CompletionModel, Message};
use rig_core::providers::openai;
use yss_harness_contract::{
    AgentDriverConfigurationFailure, AgentDriverConfigurationPort, AgentDriverFailure,
    AgentDriverFailureCode, AgentDriverPort, AgentEvent, AgentEventOutput, AgentMessage,
    AgentTurnRequest, AgentTurnResult, ApplyGraphEditRequest, AutomationCapabilityRequest,
    CancellationReason, CancellationToken, CapabilityFailure, CapabilityFailureCode, CapabilityId,
    InspectDatasetProfileRequest, InspectDatasetSchemaRequest, InspectGraphRequest,
    InspectProjectRequest, InspectResultRequest, ModelCapabilityExecutor, ModelCapabilityRequest,
    SearchNodeCatalogRequest, SecretCredential, StatisticalPlan, ToolDescriptor,
    statistical_plan_schema,
};

pub fn openai_agent_driver(
    api_key: yss_harness_contract::SecretCredential,
    base_url: impl Into<String>,
    model: impl Into<String>,
    config: RigAgentDriverConfig,
) -> Result<Arc<dyn AgentDriverPort>, RigProviderConfigurationError> {
    let base_url = base_url.into();
    let model = model.into();
    if !is_valid_base_url(&base_url) || model.trim().is_empty() || model.len() > 256 {
        return Err(RigProviderConfigurationError::Invalid);
    }
    let client = openai::Client::builder()
        .api_key(api_key.expose())
        .base_url(base_url)
        .http_client(
            rig_core::http_client::ReqwestClient::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(60))
                .build()
                .map_err(|_| RigProviderConfigurationError::Invalid)?,
        )
        .build()
        .map_err(|_| RigProviderConfigurationError::Invalid)?;
    let driver = RigAgentDriver::new(client.completion_model(model), config)
        .map_err(|_| RigProviderConfigurationError::Invalid)?;
    Ok(Arc::new(driver))
}

#[derive(Default)]
pub struct UnavailableAgentDriver;

impl AgentDriverPort for UnavailableAgentDriver {
    fn run_turn<'a>(
        &'a self,
        _request: AgentTurnRequest,
        _capabilities: Arc<dyn ModelCapabilityExecutor>,
        _output: Arc<dyn AgentEventOutput>,
        _cancellation: CancellationToken,
    ) -> yss_harness_contract::AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async { Err(provider_unavailable()) })
    }
}

/// Runtime-switchable provider adapter. The Harness keeps this stable for the
/// lifetime of the application while settings commands replace only the model
/// driver used by newly admitted turns.
pub struct ConfigurableAgentDriver {
    driver: RwLock<Arc<dyn AgentDriverPort>>,
    provider_configured: AtomicBool,
}

impl ConfigurableAgentDriver {
    pub fn new() -> Self {
        Self {
            driver: RwLock::new(Arc::new(UnavailableAgentDriver)),
            provider_configured: AtomicBool::new(false),
        }
    }

    fn set_unavailable(&self) {
        *self
            .driver
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Arc::new(UnavailableAgentDriver);
        self.provider_configured.store(false, Ordering::Release);
    }
}

impl Default for ConfigurableAgentDriver {
    fn default() -> Self {
        Self::new()
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

        let driver =
            match openai_agent_driver(credential, base_url, model, RigAgentDriverConfig::default())
            {
                Ok(driver) => driver,
                Err(_) => {
                    self.set_unavailable();
                    return Err(AgentDriverConfigurationFailure::Invalid);
                }
            };
        *self
            .driver
            .write()
            .unwrap_or_else(|error| error.into_inner()) = driver;
        self.provider_configured.store(true, Ordering::Release);
        Ok(true)
    }

    fn is_configured(&self) -> bool {
        self.provider_configured.load(Ordering::Acquire)
    }
}

fn is_valid_base_url(base_url: &str) -> bool {
    let trimmed = base_url.trim();
    (trimmed.starts_with("https://") || trimmed.starts_with("http://"))
        && trimmed.len() <= 2_048
        && !trimmed.chars().any(char::is_whitespace)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RigAgentDriverConfig {
    pub maximum_model_turns: usize,
    pub maximum_output_tokens: u64,
    pub maximum_turn_duration: Duration,
}

impl Default for RigAgentDriverConfig {
    fn default() -> Self {
        Self {
            maximum_model_turns: 16,
            maximum_output_tokens: 4_096,
            maximum_turn_duration: Duration::from_secs(180),
        }
    }
}

pub struct RigAgentDriver<M> {
    model: M,
    config: RigAgentDriverConfig,
}

impl<M> RigAgentDriver<M>
where
    M: CompletionModel + Clone + Send + Sync + 'static,
{
    pub fn new(model: M, config: RigAgentDriverConfig) -> Result<Self, RigAgentDriverConfigError> {
        if config.maximum_model_turns == 0
            || config.maximum_model_turns > 32
            || config.maximum_output_tokens == 0
            || config.maximum_output_tokens > 65_536
            || config.maximum_turn_duration.is_zero()
            || config.maximum_turn_duration > Duration::from_secs(600)
        {
            return Err(RigAgentDriverConfigError::Invalid);
        }
        Ok(Self { model, config })
    }

    async fn execute_turn(
        &self,
        request: AgentTurnRequest,
        capabilities: Arc<dyn ModelCapabilityExecutor>,
        output: Arc<dyn AgentEventOutput>,
        cancellation: CancellationToken,
    ) -> Result<AgentTurnResult, AgentDriverFailure> {
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let prepared = prepare_messages(request.messages)?;
        let tool_tasks = Arc::new(Mutex::new(Vec::new()));
        let (tool_failure, failure_receiver) = tokio::sync::watch::channel(None);
        let mut tools = request
            .tools
            .into_iter()
            .map(|descriptor| {
                dynamic_tool(
                    descriptor,
                    Arc::clone(&capabilities),
                    Arc::clone(&tool_tasks),
                    tool_failure.clone(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        tools.push(statistical_plan_tool(
            Arc::clone(&output),
            tool_failure.clone(),
        )?);
        let builder = AgentBuilder::new(self.model.clone())
            .name("yssbi-statistical-assistant")
            .preamble(&prepared.preamble)
            .default_max_turns(self.config.maximum_model_turns)
            .max_tokens(self.config.maximum_output_tokens)
            .record_content_telemetry(false);
        let agent = if tools.is_empty() {
            builder.build()
        } else {
            builder.dynamic_tools(tools).build()
        };
        let maximum_model_turns = self.config.maximum_model_turns;
        let stream_output = Arc::clone(&output);
        let stream_cancellation = cancellation.clone();
        let duration = self.config.maximum_turn_duration;
        let prompt = tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + duration;
            let stream = tokio::select! {
                stream = agent
                .stream_prompt(prepared.prompt)
                .history(prepared.history)
                .tool_concurrency(1)
                .max_turns(maximum_model_turns)
                => stream,
                _ = stream_cancellation.cancelled() => return Err(cancelled()),
                _ = tokio::time::sleep_until(deadline) => {
                    stream_cancellation.cancel(CancellationReason::DeadlineElapsed);
                    return Err(AgentDriverFailure::new(AgentDriverFailureCode::DeadlineElapsed));
                }
            };
            consume_text_stream(
                stream,
                stream_output,
                stream_cancellation,
                deadline,
                Some(failure_receiver),
            )
            .await
        });
        let mut result = prompt
            .await
            .map_err(|_| AgentDriverFailure::new(AgentDriverFailureCode::InternalFailure))
            .and_then(|result| result);
        // Stopping the model must not drop the ledger update in an admitted capability.
        // The gateway observes the same cancellation token and bounds every read-only query.
        let tasks =
            std::mem::take(&mut *tool_tasks.lock().unwrap_or_else(|error| error.into_inner()));
        for task in tasks {
            if task.await.is_err() {
                result = Err(AgentDriverFailure::new(
                    AgentDriverFailureCode::InternalFailure,
                ));
            }
        }
        if let Some(failure) = tool_failure.borrow().clone() {
            result = Err(failure);
        }
        let final_text = result?;
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        Ok(AgentTurnResult { final_text })
    }
}

async fn flush_text(
    output: &dyn AgentEventOutput,
    pending: &mut String,
) -> Result<(), AgentDriverFailure> {
    if pending.is_empty() {
        return Ok(());
    }
    output
        .emit(AgentEvent::TextDelta {
            delta: std::mem::take(pending),
        })
        .await
        .map_err(|_| AgentDriverFailure::new(AgentDriverFailureCode::OutputUnavailable))
}

async fn consume_text_stream(
    mut stream: StreamingResult,
    output: Arc<dyn AgentEventOutput>,
    cancellation: CancellationToken,
    deadline: tokio::time::Instant,
    mut tool_failures: Option<tokio::sync::watch::Receiver<Option<AgentDriverFailure>>>,
) -> Result<String, AgentDriverFailure> {
    let mut pending = String::new();
    let mut transcript = String::new();
    let mut final_seen = false;
    let mut separate_turn = false;
    let mut timer = tokio::time::interval(Duration::from_millis(40));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let result = loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break Err(cancelled()),
            _ = tokio::time::sleep_until(deadline) => {
                cancellation.cancel(CancellationReason::DeadlineElapsed);
                break Err(AgentDriverFailure::new(AgentDriverFailureCode::DeadlineElapsed));
            }
            failure = wait_tool_failure(&mut tool_failures) => break Err(failure),
            _ = timer.tick() => flush_text(output.as_ref(), &mut pending).await?,
            item = stream.next() => {
                let Some(item) = item else {
                    break if final_seen { Ok(()) } else { Err(invalid_response()) };
                };
                let item = match item {
                    Ok(item) => item,
                    Err(StreamingError::Completion(error)) => break Err(map_prompt_failure(PromptError::CompletionError(error))),
                    Err(StreamingError::Prompt(error)) => break Err(map_prompt_failure(*error)),
                };
                match item {
                    MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(text)) => {
                        if text.text.is_empty() { continue; }
                        let first = transcript.is_empty();
                        if separate_turn && !first {
                            pending.push_str("\n\n");
                            transcript.push_str("\n\n");
                        }
                        separate_turn = false;
                        if transcript.len().saturating_add(text.text.len()) > 1024 * 1024 {
                            break Err(invalid_response());
                        }
                        pending.push_str(&text.text);
                        transcript.push_str(&text.text);
                        if first || pending.len() >= 4096 { flush_text(output.as_ref(), &mut pending).await?; }
                    }
                    MultiTurnStreamItem::CompletionCall(_) => {
                        flush_text(output.as_ref(), &mut pending).await?;
                        separate_turn = true;
                    }
                    MultiTurnStreamItem::FinalResponse(_) => { final_seen = true; }
                    // No retry hooks are installed: silently retaining rejected text would corrupt the transcript.
                    MultiTurnStreamItem::ModelTurnRetried { .. } => break Err(invalid_response()),
                    _ => {
                        // Rig yields tool calls before executing them on the next poll. Publish
                        // preceding text now so Gateway tool events cannot overtake it.
                        flush_text(output.as_ref(), &mut pending).await?;
                    }
                }
            }
        }
    };
    drop(stream);
    flush_text(output.as_ref(), &mut pending).await?;
    result?;
    Ok(transcript)
}

impl<M> AgentDriverPort for RigAgentDriver<M>
where
    M: CompletionModel + Clone + Send + Sync + 'static,
{
    fn run_turn<'a>(
        &'a self,
        request: AgentTurnRequest,
        capabilities: Arc<dyn ModelCapabilityExecutor>,
        output: Arc<dyn AgentEventOutput>,
        cancellation: CancellationToken,
    ) -> yss_harness_contract::AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async move {
            self.execute_turn(request, capabilities, output, cancellation)
                .await
        })
    }
}

async fn wait_tool_failure(
    receiver: &mut Option<tokio::sync::watch::Receiver<Option<AgentDriverFailure>>>,
) -> AgentDriverFailure {
    let Some(receiver) = receiver else {
        return std::future::pending().await;
    };
    loop {
        let failure = receiver.borrow_and_update().clone();
        if let Some(failure) = failure {
            return failure;
        }
        if receiver.changed().await.is_err() {
            return AgentDriverFailure::new(AgentDriverFailureCode::InternalFailure);
        }
    }
}

fn runtime_tool_failure(
    sender: &tokio::sync::watch::Sender<Option<AgentDriverFailure>>,
    code: AgentDriverFailureCode,
) -> ToolExecutionError {
    // Rig turns ordinary tool errors into feedback. Stop fatal errors explicitly before another model step.
    sender.send_if_modified(|failure| {
        if failure.is_some() {
            return false;
        }
        *failure = Some(AgentDriverFailure::new(code));
        true
    });
    ToolExecutionError::other(code.to_string())
}

fn tool_result_json(
    outcome: Result<yss_harness_contract::AutomationCapabilityResult, CapabilityFailure>,
) -> Result<serde_json::Value, serde_json::Error> {
    match outcome {
        Ok(result) => serde_json::to_value(result),
        Err(failure) => serde_json::to_value(failure)
            .map(|failure| serde_json::json!({"state": "failed", "failure": failure})),
    }
}

fn fatal_capability_failure(code: CapabilityFailureCode) -> Option<AgentDriverFailureCode> {
    use CapabilityFailureCode::*;
    match code {
        Cancelled | ProjectSessionUnavailable | ProjectSessionMismatch | ProjectSessionChanged => {
            Some(AgentDriverFailureCode::Cancelled)
        }
        DeadlineElapsed => Some(AgentDriverFailureCode::DeadlineElapsed),
        PersistenceUnavailable | InternalFailure => Some(AgentDriverFailureCode::InternalFailure),
        InvalidRequest | GraphUnavailable | DatabaseUnavailable | CatalogUnavailable
        | ResultUnavailable | ApprovalRequired | RevisionConflict | MutationRejected
        | ResultTooLarge | InvocationConflict | GraphDraftChanged | OutcomeUnknown => None,
    }
}

struct PreparedMessages {
    preamble: String,
    history: Vec<Message>,
    prompt: Message,
}

fn prepare_messages(messages: Vec<AgentMessage>) -> Result<PreparedMessages, AgentDriverFailure> {
    let mut preamble = Vec::new();
    let mut conversation = Vec::new();
    for message in messages {
        match message {
            AgentMessage::System { content } => preamble.push(content),
            AgentMessage::User { content } => conversation.push(Message::user(content)),
            AgentMessage::Assistant { content } => conversation.push(Message::assistant(content)),
            AgentMessage::ToolCall {
                invocation_id,
                request,
            } => {
                let name = request.capability_id().as_str().to_owned();
                let mut encoded = serde_json::to_value(request).map_err(|_| invalid_response())?;
                let arguments = encoded
                    .get_mut("payload")
                    .ok_or_else(invalid_response)?
                    .take();
                use rig_core::completion::message::{
                    AssistantContent, ToolCall, ToolCallId, ToolFunction,
                };
                let call = ToolCall::new(
                    ToolCallId::new(invocation_id.to_string()).ok_or_else(invalid_response)?,
                    ToolFunction::new(name, arguments),
                );
                conversation.push(Message::Assistant {
                    id: None,
                    content: vec![AssistantContent::ToolCall(call)],
                });
            }
            AgentMessage::ToolResult {
                invocation_id,
                capability_id,
                outcome,
            } => {
                let result = tool_result_json(outcome).map_err(|_| {
                    AgentDriverFailure::new(AgentDriverFailureCode::InternalFailure)
                })?;
                conversation.push(Message::tool_result(
                    invocation_id.to_string(),
                    capability_id.as_str(),
                    result.to_string(),
                ));
            }
            AgentMessage::Plan { plan } => {
                conversation.push(Message::assistant(format!(
                    "[Statistical plan]\n{}",
                    serde_json::to_string(&plan).map_err(|_| invalid_response())?
                )));
            }
        }
    }
    let prompt = conversation.pop().ok_or_else(invalid_response)?;
    if !matches!(prompt, Message::User { .. }) {
        return Err(invalid_response());
    }
    Ok(PreparedMessages {
        preamble: preamble.join("\n\n"),
        history: conversation,
        prompt,
    })
}

fn dynamic_tool(
    descriptor: ToolDescriptor,
    capabilities: Arc<dyn ModelCapabilityExecutor>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    tool_failure: tokio::sync::watch::Sender<Option<AgentDriverFailure>>,
) -> Result<DynamicTool, AgentDriverFailure> {
    let parameters =
        serde_json::to_value(&descriptor.input_schema).map_err(|_| invalid_response())?;
    let capability_id = descriptor.capability_id;
    Ok(DynamicTool::new(
        descriptor.id.as_str(),
        tool_description(capability_id),
        parameters,
        move |_context, arguments| {
            let capabilities = Arc::clone(&capabilities);
            let tasks = Arc::clone(&tasks);
            let tool_failure = tool_failure.clone();
            Box::pin(async move {
                let request = match decode_request(capability_id, arguments) {
                    Ok(request) => request,
                    Err(_) => {
                        return tool_result_json(Err(CapabilityFailure::new(
                            CapabilityFailureCode::InvalidRequest,
                        )
                        .with_detail("reason", "tool_arguments_do_not_match_schema")))
                        .map(ToolOutput::json)
                        .map_err(|_| {
                            runtime_tool_failure(
                                &tool_failure,
                                AgentDriverFailureCode::InternalFailure,
                            )
                        });
                    }
                };
                let (sender, receiver) = tokio::sync::oneshot::channel();
                let task = tokio::spawn(async move {
                    let outcome = capabilities
                        .execute(ModelCapabilityRequest { request })
                        .await;
                    let _ = sender.send(outcome);
                });
                tasks
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .push(task);
                let outcome = receiver.await.map_err(|_| {
                    runtime_tool_failure(&tool_failure, AgentDriverFailureCode::InternalFailure)
                })?;
                if let Err(failure) = &outcome
                    && let Some(code) = fatal_capability_failure(failure.code)
                {
                    return Err(runtime_tool_failure(&tool_failure, code));
                }
                let result =
                    tool_result_json(outcome.map(|outcome| outcome.result)).map_err(|_| {
                        runtime_tool_failure(&tool_failure, AgentDriverFailureCode::InternalFailure)
                    })?;
                Ok(ToolOutput::json(result))
            })
        },
    ))
}

fn statistical_plan_tool(
    output: Arc<dyn AgentEventOutput>,
    tool_failure: tokio::sync::watch::Sender<Option<AgentDriverFailure>>,
) -> Result<DynamicTool, AgentDriverFailure> {
    let parameters =
        serde_json::to_value(statistical_plan_schema()).map_err(|_| invalid_response())?;
    Ok(DynamicTool::new(
        "propose_statistical_plan",
        "Propose a complete typed statistical plan for Harness policy validation before analytical execution. Returns accepted and the exact plan recorded by Harness after validation and persistence.",
        parameters,
        move |_context, arguments| {
            let output = Arc::clone(&output);
            let tool_failure = tool_failure.clone();
            Box::pin(async move {
                let plan = serde_json::from_value::<StatisticalPlan>(arguments).map_err(|_| {
                    ToolExecutionError::invalid_args("statistical plan did not match the schema")
                })?;
                let receipt = serde_json::json!({ "accepted": true, "plan": &plan });
                output
                    .emit(AgentEvent::PlanProposed { plan })
                    .await
                    .map_err(|failure| match failure {
                        yss_harness_contract::AgentOutputFailure::PolicyRejected => {
                            ToolExecutionError::invalid_args(
                                "statistical plan failed Harness policy validation",
                            )
                        }
                        yss_harness_contract::AgentOutputFailure::Closed
                        | yss_harness_contract::AgentOutputFailure::PersistenceFailed => {
                            runtime_tool_failure(
                                &tool_failure,
                                AgentDriverFailureCode::OutputUnavailable,
                            )
                        }
                    })?;
                Ok(ToolOutput::json(receipt))
            })
        },
    ))
}

fn decode_request(
    capability_id: CapabilityId,
    arguments: serde_json::Value,
) -> Result<AutomationCapabilityRequest, ToolExecutionError> {
    match capability_id {
        CapabilityId::InspectGraph => serde_json::from_value::<InspectGraphRequest>(arguments)
            .map(AutomationCapabilityRequest::InspectGraph),
        CapabilityId::SearchNodeCatalog => {
            serde_json::from_value::<SearchNodeCatalogRequest>(arguments)
                .map(AutomationCapabilityRequest::SearchNodeCatalog)
        }
        CapabilityId::InspectDatasetSchema => {
            serde_json::from_value::<InspectDatasetSchemaRequest>(arguments)
                .map(AutomationCapabilityRequest::InspectDatasetSchema)
        }
        CapabilityId::InspectDatasetProfile => {
            serde_json::from_value::<InspectDatasetProfileRequest>(arguments)
                .map(AutomationCapabilityRequest::InspectDatasetProfile)
        }
        CapabilityId::InspectResult => serde_json::from_value::<InspectResultRequest>(arguments)
            .map(AutomationCapabilityRequest::InspectResult),
        CapabilityId::InspectProject => serde_json::from_value::<InspectProjectRequest>(arguments)
            .map(AutomationCapabilityRequest::InspectProject),
        CapabilityId::ApplyGraphEdit => serde_json::from_value::<ApplyGraphEditRequest>(arguments)
            .map(AutomationCapabilityRequest::ApplyGraphEdit),
        CapabilityId::ValidateGraph => {
            serde_json::from_value::<yss_harness_contract::ValidateGraphRequest>(arguments)
                .map(AutomationCapabilityRequest::ValidateGraph)
        }
        CapabilityId::ExecuteGraph => {
            serde_json::from_value::<yss_harness_contract::ExecuteGraphRequest>(arguments)
                .map(AutomationCapabilityRequest::ExecuteGraph)
        }
        CapabilityId::SaveGraph => {
            serde_json::from_value::<yss_harness_contract::SaveGraphRequest>(arguments)
                .map(AutomationCapabilityRequest::SaveGraph)
        }
        CapabilityId::InspectUi => {
            serde_json::from_value(arguments).map(AutomationCapabilityRequest::InspectUi)
        }
        CapabilityId::UpdateUi => {
            serde_json::from_value(arguments).map(AutomationCapabilityRequest::UpdateUi)
        }
        CapabilityId::RequestUiIntent => {
            serde_json::from_value(arguments).map(AutomationCapabilityRequest::RequestUiIntent)
        }
        CapabilityId::ListGraphResults => {
            serde_json::from_value::<yss_harness_contract::ListGraphResultsRequest>(arguments)
                .map(AutomationCapabilityRequest::ListGraphResults)
        }
    }
    .map_err(|_| ToolExecutionError::invalid_args("tool arguments did not match the schema"))
}

fn tool_description(capability_id: CapabilityId) -> &'static str {
    match capability_id {
        CapabilityId::InspectUi => {
            "Inspect the closed UI catalog/schema, a retained linear regression result's current page and revision, or an intent receipt by ID. UI pages contain presentation only; report sections bind to actual Results. Page state lasts for the current project execution session."
        }
        CapabilityId::UpdateUi => {
            "Update a result page using the revision from inspect_ui or the latest successful update_ui. Replace the spec or apply one atomic batch of stable-element patches, change visibility, move an element within its parent, or reset. Only catalog components/actions are accepted. Send complete valid batches, never partial JSON text. Returns the committed patch with complete changed elements, removals and revision; continue from it without rereading the page when its baseRevision matches your known page. Conflicts or a missing baseline require a fresh inspection; numerical facts remain in Results."
        }
        CapabilityId::RequestUiIntent => {
            "Request opening an existing graph, focusing its node, opening a retained result, or revealing an allowed panel. Use a unique clientKey, reused only for the identical request. Pending is acceptance, not success: inspect the receipt ID until applied/failed/expired. Requires the workbench to be attached; does not edit/save project data or own FlexLayout."
        }
        CapabilityId::InspectGraph => {
            "Inspect the current Project graph by graphPath, whether or not an editor panel is open: revision, graphHash, semanticInputHash, ready, parameters, concrete port IDs/types/column names, connection limits, constants and diagnostics. Establish a baseline before editing or running; later successful edit/save receipts provide fresh facts and revisions. Inspect again when required facts are missing, the semantic baseline differs or a version conflict occurs."
        }
        CapabilityId::SearchNodeCatalog => {
            "Search node IDs, localized names, aliases and technical terms. Use concise terms (e.g. decompose, ols, multiply) or node type IDs. This searches the node catalog."
        }
        CapabilityId::InspectDatasetSchema => {
            "Inspect a bounded dataset schema and its current runtime/schema revisions."
        }
        CapabilityId::InspectDatasetProfile => {
            "Inspect bounded data-quality and shape statistics. null metrics (including duplicatedRows) mean unknown or not computed, never zero."
        }
        CapabilityId::InspectResult => {
            "Read the complete result JSON produced by YssBI, including all nested fields and data references. DataFrame/DataSeries values are paged, never expanded in full. To read a tableRef from the JSON, call inspect_result with its resultRef.executionSessionId, resultRef.resultId and part. Execution sessions change on project restart; rediscover current result references instead of reusing stale IDs. offset and limit paginate rows; use nextOffset only when hasMore is true."
        }
        CapabilityId::InspectProject => {
            "List bounded project metadata and resource identities, including graph files with no open editor panel. Use a graph resourceId as graphPath for inspect_graph and graph edits. Does not read raw dataset rows."
        }
        CapabilityId::ApplyGraphEdit => {
            "Apply one atomic, undoable batch to the current Project graph after the user requests edits. No editor panel is required. Use baseRevision and graphHash from the latest inspection or committed receipt (toRevision for edits, resourceRevision for saves); use a unique clientKey per batch. Create nodes with clientId then reference their nodeId as $clientId within that batch. Added port instances support the same $clientId in instanceId. Supports create/delete/move/duplicate nodes, parameters/literals/constants, connect/disconnect and add/remove input instances. create_constant adds a boolean/integer/decimal/string constant and its Get node; set_literal accepts a plain JSON value or null to clear. set_parameters atomically merges supplied keys with current parameters; null resets a field to its protocol default. Conditional fields are resolved by the host. Each successful batch automatically persists the complete current graph and retains undo history. File, document, history and receipt commit together; a save failure does not apply the batch. Returns toRevision, graphHash and changes with complete changed nodes/parameters/ports/derived columns, changed connections and order, removals, constants, ready and complete diagnostics. Reuse the returned facts for subsequent edits or execution without inspecting again; apply the changes only to a matching fromRevision and baseSemanticInputHash. Unchanged entities are omitted. Replayed receipts retain the original facts. Oversized receipts are rejected before commit; use smaller batches."
        }
        CapabilityId::ValidateGraph => {
            "Validate the current graph using graphHash from the latest inspection or successful edit receipt. The edit receipt already includes ready and diagnostics for its commit. Returns readiness and blocking diagnostics from editor analysis. This optional read-only check does not save, prepare an execution plan, or execute."
        }
        CapabilityId::ExecuteGraph => {
            "Execute the current graph using its current graphHash. Prepares its execution plan automatically; no prior validation call or artifact ID is required. Returns actual run status, failures and result references captured from this run's committed handoff. resultCount is the number published on success (null on failure); resultsComplete=false means the bounded references are not the complete output list. Use returned references directly with inspect_result for needed content, without listing the same results again. Later edits or runs do not change this receipt; referenced results still require availability checks. Does not save."
        }
        CapabilityId::SaveGraph => {
            "Save the current graph only when the user requests saving. Pass the current graphHash. Uses the normal Save operation and clears its undo history. Returns fromRevision, resourceRevision, graphHash and the committed dirty/canUndo/canRedo state. Use resourceRevision as the next baseRevision without an inspection solely to refresh the version. These facts describe this save, not later edits."
        }
        CapabilityId::ListGraphResults => {
            "List currently retained result IDs, run IDs and output ports for a graph, including manual runs. Use these IDs with inspect_result; do not ask the user to invent or locate an ID."
        }
    }
}

fn map_prompt_failure(error: PromptError) -> AgentDriverFailure {
    use AgentDriverFailureCode::*;
    if error
        .provider_response_json()
        .ok()
        .flatten()
        .is_some_and(|body| {
            body.pointer("/error/code")
                .and_then(serde_json::Value::as_str)
                == Some("context_length_exceeded")
        })
    {
        return AgentDriverFailure::new(ContextWindowExceeded);
    }
    let code = match error
        .provider_response_status()
        .map(|status| status.as_u16())
    {
        Some(401 | 403) => ProviderAuthenticationFailed,
        Some(429) => ProviderRateLimited,
        Some(408 | 504) => ProviderTransportFailed,
        Some(400..=499) => ProviderRequestRejected,
        Some(500..=599) => ProviderUnavailable,
        Some(_) => InvalidProviderResponse,
        None => match error {
            PromptError::CompletionError(CompletionError::HttpError(_)) => ProviderTransportFailed,
            PromptError::CompletionError(
                CompletionError::UrlError(_) | CompletionError::RequestError(_),
            ) => ProviderRequestRejected,
            PromptError::CompletionError(
                CompletionError::JsonError(_)
                | CompletionError::ResponseError(_)
                | CompletionError::ProviderResponse(_),
            )
            | PromptError::UnknownToolCall { .. } => InvalidProviderResponse,
            PromptError::CompletionError(CompletionError::ProviderError(_)) => ProviderUnavailable,
            PromptError::PromptCancelled { .. } => Cancelled,
            PromptError::MemoryError(_) | PromptError::MaxTurnsError { .. } => InternalFailure,
        },
    };
    AgentDriverFailure::new(code)
}

fn cancelled() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::Cancelled)
}

fn provider_unavailable() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::ProviderUnavailable)
}

fn invalid_response() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::InvalidProviderResponse)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RigAgentDriverConfigError {
    #[error("Rig agent driver configuration is invalid")]
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RigProviderConfigurationError {
    #[error("Rig provider configuration is invalid")]
    Invalid,
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use super::*;
    use rig_agent::completion::message::{ToolCall, ToolFunction};
    use rig_agent::streaming::StreamingCompletionResponse;
    use rig_core::completion::{
        AssistantContent, CompletionError, CompletionRequest, CompletionResponse, Usage,
    };
    use yss_harness_contract::{
        AgentFuture, AgentOutputFailure, AutomationCapabilityResult, CapabilityFailure,
        DatasetSchemaInspection, HarnessSessionId, HarnessTurnId, ModelCapabilityOutcome,
        PrincipalId, ProjectSessionBinding, ToolInvocationId,
    };
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    #[derive(Clone)]
    struct ScriptedCompletionModel {
        turns: Arc<Mutex<VecDeque<Vec<AssistantContent>>>>,
        requests: Arc<Mutex<Vec<CompletionRequest>>>,
    }

    impl ScriptedCompletionModel {
        fn new(turns: impl IntoIterator<Item = Vec<AssistantContent>>) -> Self {
            Self {
                turns: Arc::new(Mutex::new(turns.into_iter().collect())),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl CompletionModel for ScriptedCompletionModel {
        async fn completion(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, CompletionError> {
            panic!("the driver must use the streaming provider interface")
        }

        async fn stream(
            &self,
            request: CompletionRequest,
        ) -> Result<StreamingCompletionResponse, CompletionError> {
            self.requests.lock().unwrap().push(request);
            use rig_core::streaming::{RawStreamingChoice, RawStreamingToolCall, StreamFinal};
            let choice = self
                .turns
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .pop_front()
                .ok_or_else(|| CompletionError::ResponseError("script exhausted".to_owned()))?;
            let mut items = Vec::new();
            for part in choice {
                match part {
                    AssistantContent::Text(text) => {
                        items.extend(text.text.chars().map(|character| {
                            Ok(RawStreamingChoice::Message(character.to_string()))
                        }));
                    }
                    AssistantContent::ToolCall(call) => {
                        items.push(Ok(RawStreamingChoice::ToolCall(RawStreamingToolCall::new(
                            call.id.as_str(),
                            call.function.name,
                            call.function.arguments,
                        ))))
                    }
                    _ => unreachable!(),
                }
            }
            items.push(Ok(RawStreamingChoice::FinalResponse(StreamFinal::new(
                "test",
                Usage::new(),
            ))));
            Ok(StreamingCompletionResponse::stream(
                "test",
                Box::pin(futures_util::stream::iter(items)),
            ))
        }
    }

    struct StaticExecutor;

    impl ModelCapabilityExecutor for StaticExecutor {
        fn execute<'a>(
            &'a self,
            request: ModelCapabilityRequest,
        ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
            Box::pin(async move {
                assert!(matches!(
                    request.request,
                    AutomationCapabilityRequest::InspectDatasetSchema(_)
                ));
                Ok(ModelCapabilityOutcome {
                    invocation_id: ToolInvocationId::try_new("tool-1").unwrap(),
                    result: AutomationCapabilityResult::DatasetSchemaInspection(
                        DatasetSchemaInspection {
                            database_id: "database-1".to_owned(),
                            runtime_revision: 1,
                            schema_revision: 2,
                            columns: Vec::new(),
                        },
                    ),
                })
            })
        }
    }

    #[derive(Default)]
    struct CollectingOutput {
        events: Mutex<Vec<AgentEvent>>,
    }

    impl AgentEventOutput for CollectingOutput {
        fn emit<'a>(
            &'a self,
            event: AgentEvent,
        ) -> AgentFuture<'a, Result<(), AgentOutputFailure>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .push(event);
                Ok(())
            })
        }
    }

    fn request(tools: Vec<ToolDescriptor>) -> AgentTurnRequest {
        AgentTurnRequest {
            session_id: HarnessSessionId::try_new("session-1").unwrap(),
            turn_id: HarnessTurnId::try_new("turn-1").unwrap(),
            principal_id: PrincipalId::try_new("user-1").unwrap(),
            project: ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-1"),
            ),
            messages: vec![
                AgentMessage::System {
                    content: "Use evidence.".to_owned(),
                },
                AgentMessage::User {
                    content: "Inspect the schema.".to_owned(),
                },
            ],
            tools,
        }
    }

    #[test]
    fn context_capacity_failure_is_explicit_without_treating_other_rejections_as_capacity() {
        for (code, expected) in [
            (
                "context_length_exceeded",
                AgentDriverFailureCode::ContextWindowExceeded,
            ),
            (
                "invalid_request",
                AgentDriverFailureCode::ProviderRequestRejected,
            ),
        ] {
            let error = CompletionError::from_http_response(
                400u16.try_into().unwrap(),
                serde_json::json!({"error": {"code": code}}).to_string(),
            );
            assert_eq!(
                map_prompt_failure(PromptError::CompletionError(error)).code,
                expected
            );
        }
    }

    #[tokio::test]
    async fn provider_receives_complete_history_and_correlated_tool_messages() {
        let model = ScriptedCompletionModel::new([vec![AssistantContent::text("continued")]]);
        let recorded = model.requests.clone();
        let driver = RigAgentDriver::new(model, RigAgentDriverConfig::default()).unwrap();
        let text = "long-history:".to_owned() + &"h".repeat(2_000_000) + ":text-tail";
        let payload = serde_json::json!({"complete": "r".repeat(2_000_000), "tail": "result-tail"});
        let invocation_id = ToolInvocationId::try_new("saved-call-1").unwrap();
        let mut input = request(Vec::new());
        input.messages = vec![
            AgentMessage::System {
                content: "Use recorded evidence.".into(),
            },
            AgentMessage::User {
                content: text.clone(),
            },
            AgentMessage::Assistant {
                content: "Checking.".into(),
            },
            AgentMessage::ToolCall {
                invocation_id: invocation_id.clone(),
                request: AutomationCapabilityRequest::InspectResult(InspectResultRequest {
                    execution_session_id: "00000000-0000-0000-0000-000000000001".into(),
                    result_id: 17,
                    part: None,
                    offset: 0,
                    limit: 20,
                }),
            },
            AgentMessage::ToolResult {
                invocation_id,
                capability_id: CapabilityId::InspectResult,
                outcome: Ok(AutomationCapabilityResult::ResultInspection(
                    yss_harness_contract::ResultInspection {
                        result_id: 17,
                        category: yss_harness_contract::ResultCategoryInspection::Value,
                        value: yss_harness_contract::ResultValueInspection::Json(payload.clone()),
                    },
                )),
            },
            AgentMessage::Assistant {
                content: "Previous conclusion.".into(),
            },
            AgentMessage::User {
                content: "Continue.".into(),
            },
        ];
        driver
            .run_turn(
                input,
                Arc::new(StaticExecutor),
                Arc::new(CollectingOutput::default()),
                CancellationToken::default(),
            )
            .await
            .unwrap();
        let requests = recorded.lock().unwrap();
        let wire = requests[0]
            .chat_history
            .clone()
            .into_iter()
            .flat_map(|message| {
                Vec::<rig_core::providers::openai::completion::Message>::try_from(message).unwrap()
            })
            .map(|message| serde_json::to_value(message).unwrap())
            .collect::<Vec<_>>();
        let encoded = serde_json::to_string(&wire).unwrap();
        assert!(encoded.contains(&text));
        assert!(encoded.contains("result-tail"));
        let call = wire
            .iter()
            .find(|message| message.get("tool_calls").is_some())
            .unwrap();
        let result = wire
            .iter()
            .find(|message| message["role"] == "tool")
            .unwrap();
        assert_eq!(call["tool_calls"][0]["id"], result["tool_call_id"]);
        assert_eq!(call["tool_calls"][0]["function"]["name"], "inspect_result");
        let value: serde_json::Value =
            serde_json::from_str(result["content"].as_str().unwrap()).unwrap();
        assert!(value["payload"]["value"]["value"] == payload);
    }

    #[tokio::test]
    async fn rig_driver_maps_typed_tool_calls_and_emits_ordered_events() {
        let plan = serde_json::json!({
            "researchQuestion": "How does x relate to y?",
            "analysisMode": "exploratory",
            "studyDesign": { "kind": "cross_sectional", "description": "Observed data" },
            "estimands": [],
            "variableRoles": [
                { "resourceId": "database-1", "variable": "y", "role": "outcome" },
                { "resourceId": "database-1", "variable": "x", "role": "predictor" }
            ],
            "candidateMethods": ["yssbi.statistics.ols"],
            "selectedWorkflow": "dataset_quality_review",
            "requiredDiagnostics": ["measurement_scale", "missingness", "outliers", "model_assumptions", "influential_observations", "multiple_testing"],
            "robustnessChecks": [],
            "reportingContract": {
                "requireEffectSizes": true, "requireUncertainty": true,
                "requireDiagnostics": true, "requireLimitations": true, "confidenceLevel": 0.95
            }
        });
        serde_json::from_value::<StatisticalPlan>(plan.clone())
            .unwrap()
            .validate()
            .unwrap();
        struct ObservingExecutor(Arc<CollectingOutput>);
        impl ModelCapabilityExecutor for ObservingExecutor {
            fn execute<'a>(
                &'a self,
                request: ModelCapabilityRequest,
            ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
                Box::pin(async move {
                    let text = self
                        .0
                        .events
                        .lock()
                        .unwrap()
                        .iter()
                        .map(|event| match event {
                            AgentEvent::TextDelta { delta } => delta.as_str(),
                            _ => "",
                        })
                        .collect::<String>();
                    assert_eq!(text, "Checking schema.");
                    StaticExecutor.execute(request).await
                })
            }
        }
        let model = ScriptedCompletionModel::new([
            vec![
                AssistantContent::text("Checking schema."),
                AssistantContent::ToolCall(ToolCall::from_wire(
                    "call-1",
                    ToolFunction::new(
                        "inspect_dataset_schema".to_owned(),
                        serde_json::json!({ "databaseId": "database-1" }),
                    ),
                )),
            ],
            vec![AssistantContent::ToolCall(ToolCall::from_wire(
                "plan-call",
                ToolFunction::new("propose_statistical_plan".into(), plan.clone()),
            ))],
            vec![AssistantContent::text(
                "The schema inspection completed.".to_owned(),
            )],
        ]);
        let requests = model.requests.clone();
        let driver = RigAgentDriver::new(model, RigAgentDriverConfig::default()).unwrap();
        let output = Arc::new(CollectingOutput::default());

        let result = driver
            .run_turn(
                request(vec![
                    ToolDescriptor::for_capability(CapabilityId::InspectDatasetSchema).unwrap(),
                ]),
                Arc::new(ObservingExecutor(output.clone())),
                output.clone(),
                CancellationToken::default(),
            )
            .await
            .unwrap();
        let events = output
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();

        assert_eq!(
            result.final_text,
            "Checking schema.\n\nThe schema inspection completed."
        );
        assert!(events.len() > 1);
        assert_eq!(
            events
                .iter()
                .filter_map(|event| match event {
                    AgentEvent::TextDelta { delta } => Some(delta.as_str()),
                    AgentEvent::PlanProposed { .. } => None,
                    _ => panic!("unexpected event"),
                })
                .collect::<String>(),
            result.final_text
        );
        assert!(events.iter().any(|event| matches!(event, AgentEvent::PlanProposed { plan: recorded } if serde_json::to_value(recorded).unwrap() == plan)));
        let requests = requests.lock().unwrap();
        let wire = requests[2]
            .chat_history
            .clone()
            .into_iter()
            .flat_map(|message| {
                Vec::<rig_core::providers::openai::completion::Message>::try_from(message).unwrap()
            })
            .map(|message| serde_json::to_value(message).unwrap())
            .collect::<Vec<_>>();
        let receipt = wire
            .iter()
            .find(|message| message["tool_call_id"] == "plan-call")
            .unwrap();
        let receipt: serde_json::Value =
            serde_json::from_str(receipt["content"].as_str().unwrap()).unwrap();
        assert_eq!(
            receipt,
            serde_json::json!({ "accepted": true, "plan": plan })
        );
    }

    #[tokio::test]
    async fn domain_failure_is_structured_feedback_and_the_model_can_correct_its_request() {
        struct CorrectingExecutor;
        impl ModelCapabilityExecutor for CorrectingExecutor {
            fn execute<'a>(
                &'a self,
                request: ModelCapabilityRequest,
            ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
                Box::pin(async move {
                    if matches!(&request.request, AutomationCapabilityRequest::InspectDatasetSchema(request) if request.database_id == "missing")
                    {
                        return Err(CapabilityFailure::new(
                            CapabilityFailureCode::DatabaseUnavailable,
                        )
                        .with_detail("databaseId", "missing"));
                    }
                    StaticExecutor.execute(request).await
                })
            }
        }
        let call = |id: &str, database: &str| {
            vec![AssistantContent::ToolCall(ToolCall::from_wire(
                id,
                ToolFunction::new(
                    "inspect_dataset_schema".into(),
                    serde_json::json!({"databaseId": database}),
                ),
            ))]
        };
        let model = ScriptedCompletionModel::new([
            call("first", "missing"),
            call("second", "database-1"),
            vec![AssistantContent::text("Corrected and completed.")],
        ]);
        let requests = model.requests.clone();
        let driver = RigAgentDriver::new(model, RigAgentDriverConfig::default()).unwrap();
        let result = driver
            .run_turn(
                request(vec![
                    ToolDescriptor::for_capability(CapabilityId::InspectDatasetSchema).unwrap(),
                ]),
                Arc::new(CorrectingExecutor),
                Arc::new(CollectingOutput::default()),
                CancellationToken::default(),
            )
            .await;
        assert!(
            result.is_ok(),
            "ordinary tool failure must allow another model step"
        );
        assert_eq!(result.unwrap().final_text, "Corrected and completed.");
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        let wire = requests[1]
            .chat_history
            .clone()
            .into_iter()
            .flat_map(|message| {
                Vec::<rig_core::providers::openai::completion::Message>::try_from(message).unwrap()
            })
            .map(|message| serde_json::to_value(message).unwrap())
            .collect::<Vec<_>>();
        let tool = wire
            .iter()
            .find(|message| message["role"] == "tool")
            .unwrap();
        let call = wire
            .iter()
            .find(|message| message.get("tool_calls").is_some())
            .unwrap();
        assert_eq!(call["tool_calls"][0]["id"], tool["tool_call_id"]);
        let feedback = serde_json::from_str::<serde_json::Value>(tool["content"].as_str().unwrap());
        assert!(
            feedback.is_ok(),
            "domain failure must use the same structured JSON as replayed history"
        );
        let feedback = feedback.unwrap();
        assert_eq!(feedback["state"], "failed");
        assert_eq!(feedback["failure"]["code"], "database_unavailable");
        assert_eq!(feedback["failure"]["details"]["databaseId"], "missing");
    }

    #[tokio::test]
    async fn fatal_tool_failures_stop_before_another_model_request_and_settle_admitted_work() {
        struct FatalExecutor(Option<CapabilityFailureCode>, Arc<AtomicBool>);
        impl ModelCapabilityExecutor for FatalExecutor {
            fn execute<'a>(
                &'a self,
                _request: ModelCapabilityRequest,
            ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
                Box::pin(async move {
                    self.1.store(true, Ordering::SeqCst);
                    match self.0 {
                        Some(code) => Err(CapabilityFailure::new(code)),
                        None => panic!("simulated executor failure"),
                    }
                })
            }
        }
        for (failure, expected) in [
            (
                Some(CapabilityFailureCode::InternalFailure),
                AgentDriverFailureCode::InternalFailure,
            ),
            (
                Some(CapabilityFailureCode::PersistenceUnavailable),
                AgentDriverFailureCode::InternalFailure,
            ),
            (
                Some(CapabilityFailureCode::Cancelled),
                AgentDriverFailureCode::Cancelled,
            ),
            (
                Some(CapabilityFailureCode::ProjectSessionChanged),
                AgentDriverFailureCode::Cancelled,
            ),
            (
                Some(CapabilityFailureCode::DeadlineElapsed),
                AgentDriverFailureCode::DeadlineElapsed,
            ),
            (None, AgentDriverFailureCode::InternalFailure),
        ] {
            let model = ScriptedCompletionModel::new([
                vec![
                    AssistantContent::text("Before tool."),
                    AssistantContent::ToolCall(ToolCall::from_wire(
                        "fatal-call",
                        ToolFunction::new(
                            "inspect_dataset_schema".into(),
                            serde_json::json!({"databaseId": "database-1"}),
                        ),
                    )),
                ],
                vec![AssistantContent::text("Must not execute this model step.")],
            ]);
            let requests = model.requests.clone();
            let settled = Arc::new(AtomicBool::new(false));
            let output = Arc::new(CollectingOutput::default());
            let driver = RigAgentDriver::new(model, RigAgentDriverConfig::default()).unwrap();
            let result = driver
                .run_turn(
                    request(vec![
                        ToolDescriptor::for_capability(CapabilityId::InspectDatasetSchema).unwrap(),
                    ]),
                    Arc::new(FatalExecutor(failure, settled.clone())),
                    output.clone(),
                    CancellationToken::default(),
                )
                .await;
            assert_eq!(result.unwrap_err().code, expected);
            assert!(settled.load(Ordering::SeqCst));
            assert_eq!(
                requests.lock().unwrap().len(),
                1,
                "fatal failure must not reach a subsequent completion request"
            );
            let text = output
                .events
                .lock()
                .unwrap()
                .iter()
                .filter_map(|event| match event {
                    AgentEvent::TextDelta { delta } => Some(delta.clone()),
                    _ => None,
                })
                .collect::<String>();
            assert_eq!(text, "Before tool.");
        }
    }

    #[tokio::test]
    async fn text_is_published_while_provider_waits_and_pending_text_survives_cancellation() {
        use rig_core::message::Text;
        for cancel in [false, true] {
            let output = Arc::new(CollectingOutput::default());
            let token = CancellationToken::default();
            let (release, wait) = tokio::sync::oneshot::channel::<()>();
            let initial = futures_util::stream::iter([
                Ok(MultiTurnStreamItem::StreamAssistantItem(
                    StreamedAssistantContent::Text(Text::new("开始")),
                )),
                Ok(MultiTurnStreamItem::StreamAssistantItem(
                    StreamedAssistantContent::Text(Text::new("分析")),
                )),
            ]);
            let ending = futures_util::stream::once(async move {
                let _ = wait.await;
                Ok(MultiTurnStreamItem::final_response(
                    vec![AssistantContent::text("开始分析")],
                    Usage::new(),
                ))
            });
            let task = tokio::spawn(consume_text_stream(
                Box::pin(initial.chain(ending)),
                output.clone(),
                token.clone(),
                tokio::time::Instant::now() + Duration::from_secs(2),
                None,
            ));
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if output
                        .events
                        .lock()
                        .unwrap()
                        .iter()
                        .map(|event| match event {
                            AgentEvent::TextDelta { delta } => delta.as_str(),
                            _ => "",
                        })
                        .collect::<String>()
                        == "开始分析"
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .expect("text must be published before releasing the provider");
            assert!(!task.is_finished());
            if cancel {
                token.cancel(CancellationReason::User);
            } else {
                release.send(()).unwrap();
            }
            let result = task.await.unwrap();
            if cancel {
                assert_eq!(result.unwrap_err().code, AgentDriverFailureCode::Cancelled);
            } else {
                assert_eq!(result.unwrap(), "开始分析");
            }
            let events = output.events.lock().unwrap();
            assert_eq!(events.len(), 2);
        }
        let output = Arc::new(CollectingOutput::default());
        let cancellation = CancellationToken::default();
        let cancel_on_last = cancellation.clone();
        let stream = futures_util::stream::iter(["first", "pending"]).map(move |text| {
            if text == "pending" {
                cancel_on_last.cancel(CancellationReason::User);
            }
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::Text(Text::new(text)),
            ))
        });
        let result = consume_text_stream(
            Box::pin(stream),
            output.clone(),
            cancellation,
            tokio::time::Instant::now() + Duration::from_secs(1),
            None,
        )
        .await;
        assert_eq!(result.unwrap_err().code, AgentDriverFailureCode::Cancelled);
        assert_eq!(
            output.events.lock().unwrap().as_slice(),
            &[
                AgentEvent::TextDelta {
                    delta: "first".into()
                },
                AgentEvent::TextDelta {
                    delta: "pending".into()
                },
            ]
        );
    }

    #[test]
    fn cancellation_token_preserves_the_first_reason() {
        let token = CancellationToken::default();
        assert!(token.cancel(yss_harness_contract::CancellationReason::User));
        assert!(!token.cancel(yss_harness_contract::CancellationReason::DeadlineElapsed));
        assert_eq!(
            token.reason(),
            Some(yss_harness_contract::CancellationReason::User)
        );
    }

    #[tokio::test]
    async fn cancelling_a_model_turn_waits_for_admitted_tool_cleanup() {
        struct WaitingExecutor {
            cancellation: CancellationToken,
            entered: tokio::sync::Notify,
            settled: AtomicBool,
        }
        impl ModelCapabilityExecutor for WaitingExecutor {
            fn execute<'a>(
                &'a self,
                _: ModelCapabilityRequest,
            ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
                Box::pin(async move {
                    self.entered.notify_one();
                    self.cancellation.cancelled().await;
                    tokio::task::yield_now().await;
                    self.settled.store(true, Ordering::Release);
                    Err(CapabilityFailure::new(CapabilityFailureCode::Cancelled))
                })
            }
        }
        let cancellation = CancellationToken::default();
        let executor = Arc::new(WaitingExecutor {
            cancellation: cancellation.clone(),
            entered: tokio::sync::Notify::new(),
            settled: AtomicBool::new(false),
        });
        let model =
            ScriptedCompletionModel::new([vec![AssistantContent::ToolCall(ToolCall::from_wire(
                "call-1",
                ToolFunction::new(
                    "inspect_dataset_schema".into(),
                    serde_json::json!({"databaseId": "database-1"}),
                ),
            ))]]);
        let driver = RigAgentDriver::new(model, RigAgentDriverConfig::default()).unwrap();
        let (result, ()) = tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(
                driver.run_turn(
                    request(vec![
                        ToolDescriptor::for_capability(CapabilityId::InspectDatasetSchema).unwrap()
                    ]),
                    executor.clone(),
                    Arc::new(CollectingOutput::default()),
                    cancellation.clone()
                ),
                async {
                    executor.entered.notified().await;
                    cancellation.cancel(CancellationReason::User);
                }
            )
        })
        .await
        .unwrap();
        assert_eq!(result.unwrap_err().code, AgentDriverFailureCode::Cancelled);
        assert!(executor.settled.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn provider_timeout_and_panic_return_terminal_failures() {
        #[derive(Clone)]
        struct BrokenModel {
            panic: bool,
        }
        impl CompletionModel for BrokenModel {
            async fn completion(
                &self,
                _: CompletionRequest,
            ) -> Result<CompletionResponse, CompletionError> {
                assert!(!self.panic, "synthetic provider panic");
                std::future::pending().await
            }
            async fn stream(
                &self,
                _: CompletionRequest,
            ) -> Result<StreamingCompletionResponse, CompletionError> {
                assert!(!self.panic, "synthetic provider panic");
                std::future::pending().await
            }
        }
        for (panic, expected) in [
            (false, AgentDriverFailureCode::DeadlineElapsed),
            (true, AgentDriverFailureCode::InternalFailure),
        ] {
            let driver = RigAgentDriver::new(
                BrokenModel { panic },
                RigAgentDriverConfig {
                    maximum_turn_duration: Duration::from_millis(20),
                    ..RigAgentDriverConfig::default()
                },
            )
            .unwrap();
            let result = tokio::time::timeout(
                Duration::from_secs(2),
                driver.run_turn(
                    request(Vec::new()),
                    Arc::new(StaticExecutor),
                    Arc::new(CollectingOutput::default()),
                    CancellationToken::default(),
                ),
            )
            .await
            .unwrap();
            assert_eq!(result.unwrap_err().code, expected);
        }
    }

    #[test]
    fn configurable_driver_starts_unavailable_and_can_be_cleared() {
        let driver = ConfigurableAgentDriver::new();
        assert!(!driver.is_configured());
        assert!(
            !driver
                .configure(
                    "https://api.openai.com/v1".to_owned(),
                    "gpt-test".to_owned(),
                    None
                )
                .expect("clearing provider settings is valid")
        );
        assert!(!driver.is_configured());
    }

    #[tokio::test]
    async fn configured_https_provider_attempts_a_tls_handshake() {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cancellation = CancellationToken::default();
        let client = openai::Client::builder()
            .api_key("test-credential")
            .base_url(format!("https://{}/v1", listener.local_addr().unwrap()))
            .http_client(
                rig_core::http_client::ReqwestClient::builder()
                    .no_proxy()
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let driver = RigAgentDriver::new(
            client.completion_model("test-model"),
            RigAgentDriverConfig::default(),
        )
        .unwrap();
        let server = async {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut record = [0; 5];
            stream.read_exact(&mut record).await.unwrap();
            // Stop after ClientHello so the probe needs neither a trusted certificate nor an API key.
            cancellation.cancel(yss_harness_contract::CancellationReason::User);
            record
        };
        let (result, record) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(
                driver.run_turn(
                    request(vec![]),
                    Arc::new(StaticExecutor),
                    Arc::new(CollectingOutput::default()),
                    cancellation.clone()
                ),
                server,
            )
        })
        .await
        .expect("the HTTPS transport must initiate TLS");
        assert_eq!(&record[..2], &[22, 3]);
        assert_eq!(result.unwrap_err().code, AgentDriverFailureCode::Cancelled);
    }
}
