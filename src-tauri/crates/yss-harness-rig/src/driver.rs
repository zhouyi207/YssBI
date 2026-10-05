//! Run model/tool turns until completion or cancellation, using Core's tool policy.

use crate::context::ContextHook;
use crate::error::{cancelled, invalid_response};
use crate::messages::prepare_messages;
use crate::recovery::{TurnOutput, backoff, retryable};
use crate::stream::consume_text_stream_with_activity;
use crate::tools::{dynamic_tool, statistical_plan_tool, worker_tool};
use rig_agent::agent::AgentBuilder;
use rig_core::{DynModel, operation::Completion};
use std::sync::{Arc, Mutex};
use tracing::Instrument;
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentDriverPort, AgentEventOutput,
    AgentTurnRequest, AgentTurnResult, CancellationToken, ModelCapabilityExecutor,
};

pub struct RigAgentDriver {
    model: DynModel<Completion>,
    context_window: Option<u32>,
    max_output_tokens: Option<u32>,
    temperature: Option<f64>,
    additional_parameters: serde_json::Value,
}

impl RigAgentDriver {
    pub fn new(model: impl Into<DynModel<Completion>>) -> Self {
        Self {
            model: model.into(),
            context_window: None,
            max_output_tokens: None,
            temperature: None,
            additional_parameters: serde_json::json!({}),
        }
    }

    pub(crate) fn with_model_config(
        mut self,
        config: &yss_harness_contract::LanguageModelConfig,
        protocol: yss_harness_contract::LanguageModelProtocol,
    ) -> Self {
        self.context_window = config.context_window;
        self.max_output_tokens = config.max_output_tokens;
        self.temperature = config.temperature;
        let mut parameters = config.additional_parameters.clone();
        if let Some(top_p) = config.top_p {
            if protocol == yss_harness_contract::LanguageModelProtocol::Gemini {
                let generation = parameters
                    .entry("generation_config")
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(generation) = generation.as_object_mut() {
                    generation.insert("top_p".into(), top_p.into());
                }
            } else {
                parameters.insert("top_p".into(), top_p.into());
            }
        }
        if protocol == yss_harness_contract::LanguageModelProtocol::Gemini {
            // Conversation state and replay belong to the Harness.
            parameters.insert("store".into(), false.into());
        }
        self.additional_parameters = parameters.into();
        self
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
        if request.tool_concurrency == 0 {
            return Err(invalid_response());
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
        if request
            .control_tools
            .contains(&yss_harness_contract::AgentControlTool::ProposeStatisticalPlan)
        {
            tools.push(statistical_plan_tool(
                Arc::clone(&output),
                tool_failure.clone(),
            )?);
        }
        for control in &request.control_tools {
            if matches!(
                control,
                yss_harness_contract::AgentControlTool::DelegateTask
                    | yss_harness_contract::AgentControlTool::FollowupTask
            ) {
                tools.push(worker_tool(
                    *control == yss_harness_contract::AgentControlTool::FollowupTask,
                    Arc::clone(&capabilities),
                    Arc::clone(&tool_tasks),
                    tool_failure.clone(),
                )?);
            }
        }
        let output = Arc::new(TurnOutput::new(output));
        let hook = ContextHook::new(
            self.model.clone(),
            prepared.clone(),
            output.clone(),
            capabilities.clone(),
            cancellation.clone(),
            self.context_window,
            self.max_output_tokens,
        )
        .with_model_parameters(self.temperature, self.additional_parameters.clone());
        let mut builder = AgentBuilder::new(self.model.clone())
            .name(request.role.name())
            .preamble(&prepared.preamble)
            // Rig otherwise defaults to one model call. The Harness ends on the
            // model's final response or cancellation, not a product turn quota.
            .default_max_turns(usize::MAX)
            .record_content_telemetry(false)
            .add_hook(hook.clone());
        if let Some(tokens) = self.max_output_tokens {
            builder = builder.max_tokens(u64::from(tokens));
        }
        if let Some(temperature) = self.temperature {
            builder = builder.temperature(temperature);
        }
        builder = builder.additional_params(self.additional_parameters.clone());
        let agent = builder.dynamic_tools(tools).build();
        let stream_output = Arc::clone(&output);
        let stream_cancellation = cancellation.clone();
        let concurrency = request.tool_concurrency;
        let prompt = tokio::spawn(
            async move {
                let mut completed_calls = 0;
                let mut attempt = 0;
                loop {
                    let boundary = hook.boundary();
                    let stream = agent
                        .prompt(boundary.messages.prompt)
                        .history(boundary.messages.history)
                        .tool_concurrency(concurrency)
                        .stream();
                    let result = consume_text_stream_with_activity(
                        stream,
                        stream_output.clone(),
                        stream_cancellation.clone(),
                        failure_receiver.clone(),
                        request.output_mode == yss_harness_contract::AgentOutputMode::FinalResponse,
                        hook.activity.clone(),
                    )
                    .await;
                    hook.activity.finish();
                    if hook.take_compacted() {
                        continue;
                    }
                    let result = hook.take_failure().map_or(result, Err);
                    let error = match result {
                        Ok(final_text) => {
                            return Ok(
                                if request.output_mode
                                    == yss_harness_contract::AgentOutputMode::FinalResponse
                                {
                                    final_text
                                } else {
                                    stream_output.text()
                                },
                            );
                        }
                        Err(error) => error,
                    };
                    let boundary = hook.boundary();
                    if boundary.completed_calls != completed_calls {
                        completed_calls = boundary.completed_calls;
                        attempt = 0;
                    }
                    let context_exceeded =
                        error.code == AgentDriverFailureCode::ContextWindowExceeded;
                    if (!retryable(error.code) && !context_exceeded) || attempt >= 5 {
                        return Err(error);
                    }
                    // A checkpoint is taken only after all prior tool outcomes entered Rig's
                    // history. Reissuing this sampling request cannot replay their execution.
                    stream_output.rewind(boundary.text_position).await?;
                    use yss_harness_contract::{AgentEvent, AgentRuntimePhase};
                    stream_output
                        .emit(AgentEvent::RuntimeStatus {
                            phase: if context_exceeded {
                                AgentRuntimePhase::Compacting
                            } else {
                                AgentRuntimePhase::Reconnecting
                            },
                            attempt: attempt + 1,
                        })
                        .await
                        .map_err(|_| {
                            AgentDriverFailure::new(AgentDriverFailureCode::OutputUnavailable)
                        })?;
                    hook.restart(context_exceeded);
                    backoff(&error, attempt, &stream_cancellation).await?;
                    attempt += 1;
                }
            }
            .instrument(tracing::Span::current()),
        );
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

impl AgentDriverPort for RigAgentDriver {
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
