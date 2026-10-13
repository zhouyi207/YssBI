//! Map Harness tool declarations into Rig tools. Calls and results stay opaque JSON.

use rig_agent::tool::{DynamicTool, ToolExecutionError, ToolOutput};
use std::sync::{Arc, Mutex};
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentToolExecutor, ToolCall, ToolSpec,
};

mod feedback;
pub(crate) use feedback::ToolCallFeedback;

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

pub(crate) fn dynamic_tool(
    spec: ToolSpec,
    executor: Arc<dyn AgentToolExecutor>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    tool_failure: tokio::sync::watch::Sender<Option<AgentDriverFailure>>,
) -> DynamicTool {
    let name = spec.name.clone();
    DynamicTool::new(spec.name, spec.description, spec.input_schema, move |arguments| {
        let name = name.clone();
        let executor = Arc::clone(&executor);
        let tasks = Arc::clone(&tasks);
        let tool_failure = tool_failure.clone();
        Box::pin(async move {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            // The Harness settles every accepted call, including after the model stream ends.
            let task = tokio::spawn(async move {
                let outcome = executor.call(ToolCall { name, arguments }).await;
                if sender.send(outcome).is_err() {
                    tracing::debug!(
                        domain = "Harness",
                        event = "harness_tool_result_unobserved",
                        "Rig stopped waiting for a settled tool call"
                    );
                }
            });
            tasks
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(task);
            let outcome = receiver.await.map_err(|_| {
                runtime_tool_failure(&tool_failure, AgentDriverFailureCode::InternalFailure)
            })?;
            if let Some(code) = outcome.fatal {
                return Err(runtime_tool_failure(&tool_failure, code));
            }
            Ok(ToolOutput::json(outcome.output))
        })
    })
}

