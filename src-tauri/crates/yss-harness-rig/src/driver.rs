//! Execute a provider-neutral request with the limits and tools supplied by Core.

use crate::error::{cancelled, invalid_response};
use crate::messages::prepare_messages;
use crate::stream::consume_text_stream;
use crate::tools::{delegation_tool, dynamic_tool, statistical_plan_tool};
use rig_agent::agent::AgentBuilder;
use rig_core::completion::CompletionModel;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentDriverPort, AgentEventOutput,
    AgentTurnRequest, AgentTurnResult, CancellationReason, CancellationToken,
    ModelCapabilityExecutor,
};

use rig_agent::streaming::StreamingPrompt;

pub struct RigAgentDriver<M> {
    model: M,
}

impl<M> RigAgentDriver<M>
where
    M: CompletionModel + Clone + Send + Sync + 'static,
{
    pub fn new(model: M) -> Self {
        Self { model }
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
        if request.limits.maximum_model_turns == 0
            || request.limits.maximum_output_tokens == 0
            || request.limits.maximum_duration_ms == 0
            || request.limits.tool_concurrency == 0
        {
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
        if request
            .control_tools
            .contains(&yss_harness_contract::AgentControlTool::DelegateTask)
        {
            tools.push(delegation_tool(
                Arc::clone(&capabilities),
                Arc::clone(&tool_tasks),
                tool_failure.clone(),
            )?);
        }
        let builder = AgentBuilder::new(self.model.clone())
            .name(request.role.name())
            .preamble(&prepared.preamble)
            .default_max_turns(request.limits.maximum_model_turns)
            .record_content_telemetry(false);
        let builder = builder.max_tokens(request.limits.maximum_output_tokens);
        let agent = builder.dynamic_tools(tools).build();
        let stream_output = Arc::clone(&output);
        let stream_cancellation = cancellation.clone();
        let duration = Duration::from_millis(request.limits.maximum_duration_ms);
        let concurrency = request.limits.tool_concurrency;
        let prompt = tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + duration;
            let stream = tokio::select! {
                stream = agent
                .stream_prompt(prepared.prompt)
                .history(prepared.history)
                .tool_concurrency(concurrency)
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
                failure_receiver,
                request.output_mode == yss_harness_contract::AgentOutputMode::FinalResponse,
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
