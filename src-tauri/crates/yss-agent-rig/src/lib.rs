//! Rig adapter for the provider-neutral Harness Agent Driver port.

#![forbid(unsafe_code)]

use std::future::IntoFuture;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};

use rig_agent::agent::AgentBuilder;
use rig_agent::completion::Prompt;
use rig_agent::core::client::CompletionClient;
use rig_agent::core::completion::{CompletionModel, Message};
use rig_agent::core::providers::openai;
use rig_agent::tool::{DynamicTool, ToolExecutionError, ToolOutput};
use yss_automation_contract::{
    AgentDriverConfigurationFailure, AgentDriverConfigurationPort, AgentDriverFailure,
    AgentDriverFailureCode, AgentDriverPort, AgentEvent, AgentEventOutput, AgentMessage,
    AgentMessageRole, AgentTurnRequest, AgentTurnResult, ApplyGraphEditRequest,
    AutomationCapabilityRequest, CancellationToken, CapabilityFailure, CapabilityFailureCode,
    CapabilityId, InspectDatasetProfileRequest, InspectDatasetSchemaRequest, InspectGraphRequest,
    InspectProjectRequest, InspectResultRequest, ModelCapabilityExecutor, ModelCapabilityRequest,
    SearchNodeCatalogRequest, SecretCredential, StatisticalPlan, ToolDescriptor,
    statistical_plan_schema,
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
}

impl Default for RigAgentDriverConfig {
    fn default() -> Self {
        Self {
            maximum_model_turns: 8,
            maximum_output_tokens: 4_096,
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
        let mut tools = request
            .tools
            .into_iter()
            .map(|descriptor| {
                dynamic_tool(descriptor, Arc::clone(&capabilities), Arc::clone(&output))
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
        let prompt = agent
            .prompt(prepared.prompt)
            .history(prepared.history)
            .tool_concurrency(1)
            .max_turns(self.config.maximum_model_turns)
            .into_future();
        let final_text = tokio::select! {
            result = prompt => result.map_err(|_| provider_unavailable())?,
            _ = cancellation.cancelled() => return Err(cancelled()),
        };
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
    output: Arc<dyn AgentEventOutput>,
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
            let output = Arc::clone(&output);
            Box::pin(async move {
                let request = decode_request(capability_id, arguments)?;
                output
                    .emit(AgentEvent::ToolInvocationRequested { capability_id })
                    .await
                    .map_err(map_output_failure)?;
                let outcome = capabilities
                    .execute(ModelCapabilityRequest { request })
                    .await
                    .map_err(map_capability_failure)?;
                output
                    .emit(AgentEvent::ToolInvocationCompleted {
                        invocation_id: outcome.invocation_id,
                        capability_id,
                    })
                    .await
                    .map_err(map_output_failure)?;
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
    }
    .map_err(|_| ToolExecutionError::invalid_args("tool arguments did not match the schema"))
}

fn map_capability_failure(failure: CapabilityFailure) -> ToolExecutionError {
    let code = failure.code.to_string();
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
        CapabilityFailureCode::RevisionConflict | CapabilityFailureCode::MutationRejected => {
            ToolExecutionError::other(code).with_retryable(false)
        }
        CapabilityFailureCode::InvocationConflict
        | CapabilityFailureCode::PersistenceUnavailable
        | CapabilityFailureCode::InternalFailure => ToolExecutionError::other(code),
    }
    .with_code(failure.code.to_string())
}

fn tool_description(capability_id: CapabilityId) -> &'static str {
    match capability_id {
        CapabilityId::InspectGraph => {
            "Inspect a bounded project graph snapshot with typed nodes and connections."
        }
        CapabilityId::SearchNodeCatalog => {
            "Search the localized YssBI node catalog without mutating the project."
        }
        CapabilityId::InspectDatasetSchema => {
            "Inspect a bounded dataset schema and its current runtime/schema revisions."
        }
        CapabilityId::InspectDatasetProfile => {
            "Inspect bounded data-quality and shape statistics for a current dataset revision."
        }
        CapabilityId::InspectResult => {
            "Inspect a bounded structured execution result produced by YssBI."
        }
        CapabilityId::InspectProject => {
            "Inspect bounded project metadata and resource identities without reading raw data."
        }
        CapabilityId::ApplyGraphEdit => {
            "Apply one approved revision-aware graph edit batch with one durable commit."
        }
    }
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
    use rig_agent::core::completion::{
        AssistantContent, CompletionError, CompletionRequest, CompletionResponse, Usage,
    };
    use rig_agent::streaming::StreamingCompletionResponse;
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
        assert!(matches!(
            events.as_slice(),
            [
                AgentEvent::ToolInvocationRequested {
                    capability_id: CapabilityId::InspectDatasetSchema
                },
                AgentEvent::ToolInvocationCompleted {
                    capability_id: CapabilityId::InspectDatasetSchema,
                    ..
                },
                AgentEvent::TextDelta { .. }
            ]
        ));
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
}
