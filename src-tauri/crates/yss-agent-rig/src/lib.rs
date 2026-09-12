//! Rig adapter for the provider-neutral Harness Agent Driver port.

#![forbid(unsafe_code)]

use std::future::IntoFuture;
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use rig_agent::agent::AgentBuilder;
use rig_agent::completion::{Prompt, PromptError};
use rig_agent::tool::{DynamicTool, ToolExecutionError, ToolOutput};
use rig_core::client::CompletionClient;
use rig_core::completion::{CompletionError, CompletionModel, Message};
use rig_core::providers::openai;
use yss_automation_contract::{
    AgentDriverConfigurationFailure, AgentDriverConfigurationPort, AgentDriverFailure,
    AgentDriverFailureCode, AgentDriverPort, AgentEvent, AgentEventOutput, AgentMessage,
    AgentMessageRole, AgentTurnRequest, AgentTurnResult, ApplyGraphEditRequest,
    AutomationCapabilityRequest, CancellationReason, CancellationToken, CapabilityFailure,
    CapabilityFailureCode, CapabilityId, InspectDatasetProfileRequest, InspectDatasetSchemaRequest,
    InspectGraphRequest, InspectProjectRequest, InspectResultRequest, ModelCapabilityExecutor,
    ModelCapabilityRequest, SearchNodeCatalogRequest, SecretCredential, StatisticalPlan,
    ToolDescriptor, statistical_plan_schema,
};

pub fn openai_agent_driver(
    api_key: yss_automation_contract::SecretCredential,
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
    ) -> yss_automation_contract::AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
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
    ) -> yss_automation_contract::AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
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
        let mut tools = request
            .tools
            .into_iter()
            .map(|descriptor| {
                dynamic_tool(
                    descriptor,
                    Arc::clone(&capabilities),
                    Arc::clone(&tool_tasks),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        tools.push(statistical_plan_tool(Arc::clone(&output))?);
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
        let mut prompt = tokio::spawn(async move {
            agent
                .prompt(prepared.prompt)
                .history(prepared.history)
                .tool_concurrency(1)
                .max_turns(maximum_model_turns)
                .into_future()
                .await
        });
        let mut result = tokio::select! {
            result = &mut prompt => result
                .map_err(|_| AgentDriverFailure::new(AgentDriverFailureCode::InternalFailure))
                .and_then(|result| result.map_err(map_prompt_failure)),
            _ = cancellation.cancelled() => {
                prompt.abort();
                let _ = prompt.await;
                Err(cancelled())
            },
            _ = tokio::time::sleep(self.config.maximum_turn_duration) => {
                cancellation.cancel(CancellationReason::DeadlineElapsed);
                prompt.abort();
                let _ = prompt.await;
                Err(AgentDriverFailure::new(AgentDriverFailureCode::DeadlineElapsed))
            },
        };
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
        let final_text = result?;
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        output
            .emit(AgentEvent::TextDelta {
                delta: final_text.clone(),
            })
            .await
            .map_err(|_| AgentDriverFailure::new(AgentDriverFailureCode::OutputUnavailable))?;
        Ok(AgentTurnResult { final_text })
    }
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
    ) -> yss_automation_contract::AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async move {
            self.execute_turn(request, capabilities, output, cancellation)
                .await
        })
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
        if message.content.trim().is_empty() || message.content.len() > 1024 * 1024 {
            return Err(invalid_response());
        }
        match message.role {
            AgentMessageRole::System => preamble.push(message.content),
            AgentMessageRole::User => conversation.push(Message::user(message.content)),
            AgentMessageRole::Assistant => conversation.push(Message::assistant(message.content)),
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
            Box::pin(async move {
                let request = decode_request(capability_id, arguments)?;
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
                let outcome = receiver
                    .await
                    .map_err(|_| ToolExecutionError::other("tool execution interrupted"))?
                    .map_err(map_capability_failure)?;
                let result = serde_json::to_value(outcome.result)
                    .map_err(|_| ToolExecutionError::other("tool result encoding failed"))?;
                Ok(ToolOutput::json(result))
            })
        },
    ))
}

fn statistical_plan_tool(
    output: Arc<dyn AgentEventOutput>,
) -> Result<DynamicTool, AgentDriverFailure> {
    let parameters =
        serde_json::to_value(statistical_plan_schema()).map_err(|_| invalid_response())?;
    Ok(DynamicTool::new(
        "propose_statistical_plan",
        "Propose a complete typed statistical plan for Harness policy validation before analytical execution.",
        parameters,
        move |_context, arguments| {
            let output = Arc::clone(&output);
            Box::pin(async move {
                let plan = serde_json::from_value::<StatisticalPlan>(arguments).map_err(|_| {
                    ToolExecutionError::invalid_args("statistical plan did not match the schema")
                })?;
                output
                    .emit(AgentEvent::PlanProposed { plan })
                    .await
                    .map_err(map_output_failure)?;
                Ok(ToolOutput::json(serde_json::json!({ "accepted": true })))
            })
        },
    ))
}

fn map_output_failure(failure: yss_automation_contract::AgentOutputFailure) -> ToolExecutionError {
    match failure {
        yss_automation_contract::AgentOutputFailure::PolicyRejected => {
            ToolExecutionError::invalid_args("statistical plan failed Harness policy validation")
        }
        yss_automation_contract::AgentOutputFailure::Closed
        | yss_automation_contract::AgentOutputFailure::PersistenceFailed => {
            ToolExecutionError::other("tool event output unavailable")
        }
    }
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
        CapabilityId::CompileGraph => {
            serde_json::from_value::<yss_automation_contract::CompileGraphRequest>(arguments)
                .map(AutomationCapabilityRequest::CompileGraph)
        }
        CapabilityId::ExecuteGraph => {
            serde_json::from_value::<yss_automation_contract::ExecuteGraphRequest>(arguments)
                .map(AutomationCapabilityRequest::ExecuteGraph)
        }
        CapabilityId::SaveGraph => {
            serde_json::from_value::<yss_automation_contract::SaveGraphRequest>(arguments)
                .map(AutomationCapabilityRequest::SaveGraph)
        }
        CapabilityId::ListGraphResults => {
            serde_json::from_value::<yss_automation_contract::ListGraphResultsRequest>(arguments)
                .map(AutomationCapabilityRequest::ListGraphResults)
        }
    }
    .map_err(|_| ToolExecutionError::invalid_args("tool arguments did not match the schema"))
}

fn map_capability_failure(failure: CapabilityFailure) -> ToolExecutionError {
    let code = failure.details.get("reason").map_or_else(
        || failure.code.to_string(),
        |reason| format!("{}: {reason}", failure.code),
    );
    match failure.code {
        CapabilityFailureCode::InvalidRequest | CapabilityFailureCode::ResultTooLarge => {
            ToolExecutionError::invalid_args(code)
        }
        CapabilityFailureCode::Cancelled => ToolExecutionError::cancelled(code),
        CapabilityFailureCode::DeadlineElapsed => ToolExecutionError::timeout(code),
        CapabilityFailureCode::GraphUnavailable
        | CapabilityFailureCode::DatabaseUnavailable
        | CapabilityFailureCode::CatalogUnavailable
        | CapabilityFailureCode::ResultUnavailable => ToolExecutionError::not_found(code),
        CapabilityFailureCode::ProjectSessionMismatch
        | CapabilityFailureCode::ProjectSessionChanged
        | CapabilityFailureCode::ProjectSessionUnavailable
        | CapabilityFailureCode::ApprovalRequired => ToolExecutionError::permission_denied(code),
        CapabilityFailureCode::RevisionConflict
        | CapabilityFailureCode::MutationRejected
        | CapabilityFailureCode::OutcomeUnknown => {
            ToolExecutionError::other(code).with_retryable(false)
        }
        CapabilityFailureCode::InvocationConflict
        | CapabilityFailureCode::PersistenceUnavailable
        | CapabilityFailureCode::InternalFailure
        | CapabilityFailureCode::GraphClientUnavailable
        | CapabilityFailureCode::GraphDraftChanged
        | CapabilityFailureCode::GraphCompileFailed
        | CapabilityFailureCode::GraphExecutionFailed => ToolExecutionError::other(code),
    }
    .with_code(failure.code.to_string())
}

fn tool_description(capability_id: CapabilityId) -> &'static str {
    match capability_id {
        CapabilityId::InspectGraph => {
            "Inspect the current editor draft: revision, graphHash, parameters, concrete port IDs/types/column names, connection limits, constants and diagnostics. Inspect before editing or compiling."
        }
        CapabilityId::SearchNodeCatalog => {
            "Search node IDs, localized names, aliases and technical terms. Use concise terms (e.g. decompose, ols, multiply) or node type IDs. This searches nodes, not compile/run commands."
        }
        CapabilityId::InspectDatasetSchema => {
            "Inspect a bounded dataset schema and its current runtime/schema revisions."
        }
        CapabilityId::InspectDatasetProfile => {
            "Inspect bounded data-quality and shape statistics. null metrics (including duplicatedRows) mean unknown or not computed, never zero."
        }
        CapabilityId::InspectResult => {
            "Inspect a bounded structured execution result produced by YssBI. For table/series previews, offset and limit paginate rows; use nextOffset only when hasMore is true. Lists/records indicate truncation explicitly."
        }
        CapabilityId::InspectProject => {
            "Inspect bounded project metadata and resource identities without reading raw data."
        }
        CapabilityId::ApplyGraphEdit => {
            "Apply one atomic, undoable batch to the current editor draft after the user requests edits. Use revision as baseRevision and graphHash from inspect_graph; use a unique clientKey per batch. Create nodes with clientId then reference their nodeId as $clientId within that batch. Added port instances support the same $clientId in instanceId. Supports create/delete/move/duplicate nodes, parameters/configuration/literals/constants, connect/disconnect and add/remove input instances. create_constant adds a boolean/integer/decimal/string constant and its Get node; set_literal accepts a plain JSON value or null to clear. set_parameters merges supplied keys with existing parameters. This updates the canvas, not the saved file; save_graph is separate."
        }
        CapabilityId::CompileGraph => {
            "Compile the inspected current draft, passing its graphHash. Returns ready/artifactId or concrete blocking diagnostics. Does not save or execute."
        }
        CapabilityId::ExecuteGraph => {
            "Execute the current draft using the matching artifactId from compile_graph and current graphHash. Returns actual run status, failures and result IDs; inspect_result reads those results. Does not implicitly compile or save."
        }
        CapabilityId::SaveGraph => {
            "Save the current draft only when the user requests saving. Pass the current graphHash. Uses the editor save operation and clears its draft undo history as a normal Save does."
        }
        CapabilityId::ListGraphResults => {
            "List currently retained result IDs, run IDs and output ports for a graph, including manual runs. Use these IDs with inspect_result; do not ask the user to invent or locate an ID."
        }
    }
}

fn map_prompt_failure(error: PromptError) -> AgentDriverFailure {
    use AgentDriverFailureCode::*;
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
    use yss_automation_contract::{
        AgentFuture, AgentOutputFailure, AutomationCapabilityResult, CapabilityFailure,
        DatasetSchemaInspection, HarnessSessionId, HarnessTurnId, ModelCapabilityOutcome,
        PrincipalId, ProjectSessionBinding, ToolInvocationId,
    };
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    #[derive(Clone)]
    struct ScriptedCompletionModel {
        turns: Arc<Mutex<VecDeque<Vec<AssistantContent>>>>,
    }

    impl ScriptedCompletionModel {
        fn new(turns: impl IntoIterator<Item = Vec<AssistantContent>>) -> Self {
            Self {
                turns: Arc::new(Mutex::new(turns.into_iter().collect())),
            }
        }
    }

    impl CompletionModel for ScriptedCompletionModel {
        async fn completion(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, CompletionError> {
            let choice = self
                .turns
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .pop_front()
                .ok_or_else(|| CompletionError::ResponseError("script exhausted".to_owned()))?;
            Ok(CompletionResponse::new(choice, Usage::new(), "test"))
        }

        async fn stream(
            &self,
            _request: CompletionRequest,
        ) -> Result<StreamingCompletionResponse, CompletionError> {
            Err(CompletionError::ResponseError(
                "streaming is not used by this adapter test".to_owned(),
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
                AgentMessage {
                    role: AgentMessageRole::System,
                    content: "Use evidence.".to_owned(),
                },
                AgentMessage {
                    role: AgentMessageRole::User,
                    content: "Inspect the schema.".to_owned(),
                },
            ],
            tools,
        }
    }

    #[tokio::test]
    async fn rig_driver_maps_typed_tool_calls_and_emits_ordered_events() {
        let model = ScriptedCompletionModel::new([
            vec![AssistantContent::ToolCall(ToolCall::from_wire(
                "call-1",
                ToolFunction::new(
                    "inspect_dataset_schema".to_owned(),
                    serde_json::json!({ "databaseId": "database-1" }),
                ),
            ))],
            vec![AssistantContent::text(
                "The schema inspection completed.".to_owned(),
            )],
        ]);
        let driver = RigAgentDriver::new(model, RigAgentDriverConfig::default()).unwrap();
        let output = Arc::new(CollectingOutput::default());

        let result = driver
            .run_turn(
                request(vec![
                    ToolDescriptor::for_capability(CapabilityId::InspectDatasetSchema).unwrap(),
                ]),
                Arc::new(StaticExecutor),
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

        assert_eq!(result.final_text, "The schema inspection completed.");
        assert!(matches!(events.as_slice(), [AgentEvent::TextDelta { .. }]));
    }

    #[test]
    fn cancellation_token_preserves_the_first_reason() {
        let token = CancellationToken::default();
        assert!(token.cancel(yss_automation_contract::CancellationReason::User));
        assert!(!token.cancel(yss_automation_contract::CancellationReason::DeadlineElapsed));
        assert_eq!(
            token.reason(),
            Some(yss_automation_contract::CancellationReason::User)
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
                unreachable!()
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
        assert_eq!(
            driver
                .configure(
                    "https://api.openai.com/v1".to_owned(),
                    "gpt-test".to_owned(),
                    None
                )
                .expect("clearing provider settings is valid"),
            false
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
            cancellation.cancel(yss_automation_contract::CancellationReason::User);
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
