//! Run model/tool turns until completion or cancellation, using Core's tool policy.

use crate::error::{cancelled, invalid_response};
use crate::messages::prepare_messages;
use crate::stream::consume_text_stream;
use crate::tools::{delegation_tool, dynamic_tool, statistical_plan_tool};
use rig_agent::agent::AgentBuilder;
use rig_core::completion::CompletionModel;
use std::sync::{Arc, Mutex};
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentDriverPort, AgentEventOutput,
    AgentTurnRequest, AgentTurnResult, CancellationToken, ModelCapabilityExecutor,
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
            // Rig otherwise defaults to one model call. The Harness ends on the
            // model's final response or cancellation, not a product turn quota.
            .default_max_turns(usize::MAX)
            .record_content_telemetry(false);
        let agent = builder.dynamic_tools(tools).build();
        let stream_output = Arc::clone(&output);
        let stream_cancellation = cancellation.clone();
        let concurrency = request.tool_concurrency;
        let prompt = tokio::spawn(async move {
            let stream = tokio::select! {
                stream = agent
                .stream_prompt(prepared.prompt)
                .history(prepared.history)
                .tool_concurrency(concurrency)
                => stream,
                _ = stream_cancellation.cancelled() => return Err(cancelled()),
            };
            consume_text_stream(
                stream,
                stream_output,
                stream_cancellation,
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
