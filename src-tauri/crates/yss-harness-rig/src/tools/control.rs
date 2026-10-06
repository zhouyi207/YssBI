//! Control calls share Core's durable lifecycle, including decode and policy failures.

use super::{fatal_capability_failure, runtime_tool_failure};
use crate::{arguments, error::invalid_response, messages::tool_result_json};
use rig_agent::tool::{DynamicTool, ToolExecutionError, ToolOutput};
use std::{
    future::poll_fn,
    panic::AssertUnwindSafe,
    sync::{Arc, Mutex},
    task::Poll,
};
use yss_harness_contract::*;

enum Failure {
    Capability(CapabilityFailure),
    Output(AgentOutputFailure),
}

impl From<CapabilityFailure> for Failure {
    fn from(value: CapabilityFailure) -> Self {
        Self::Capability(value)
    }
}

impl Failure {
    fn capability_failure(&self) -> CapabilityFailure {
        match self {
            Self::Capability(failure) => failure.clone(),
            Self::Output(AgentOutputFailure::PolicyRejected { reason, .. }) => {
                CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
                    .with_detail("reason", reason)
            }
            Self::Output(_) => {
                CapabilityFailure::new(CapabilityFailureCode::PersistenceUnavailable)
            }
        }
    }
}

pub(crate) fn control_tool(
    tool: AgentControlTool,
    capabilities: Arc<dyn ModelCapabilityExecutor>,
    output: Arc<dyn AgentEventOutput>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    tool_failure: tokio::sync::watch::Sender<Option<AgentDriverFailure>>,
) -> Result<DynamicTool, AgentDriverFailure> {
    let (description, schema) = match tool {
        AgentControlTool::DelegateTask => (
            "Delegate a task with precise resources, permissions and completion criteria. Always provide worker, objective, constraints (a string), completionCriteria, dependsOn and scope as top-level fields. The host captures read observations and deduplicates identical task specifications within this turn. Use followup_task to continue a previous worker.",
            serde_json::to_value(agent_task_schema()),
        ),
        AgentControlTool::FollowupTask => (
            "Continue an existing worker by runId, keeping its history, checkpoints and committed receipts. Inspect changed inputs before resuming; the host binds observed facts within the original grant. Use after partial completion or interruption instead of creating a fresh worker.",
            serde_json::to_value(agent_followup_schema()),
        ),
        AgentControlTool::ProposeStatisticalPlan => (
            "Propose a complete typed statistical plan for Harness policy validation before analytical execution. Returns accepted and the exact plan recorded by Harness after validation and persistence.",
            serde_json::to_value(statistical_plan_schema()),
        ),
    };
    let schema = Arc::new(schema.map_err(|_| invalid_response())?);
    Ok(DynamicTool::new(
        tool.as_str(),
        description,
        (*schema).clone(),
        move |arguments| {
            let (schema, capabilities, output, tasks, tool_failure) = (
                schema.clone(),
                capabilities.clone(),
                output.clone(),
                tasks.clone(),
                tool_failure.clone(),
            );
            Box::pin(async move {
                let (sender, receiver) = tokio::sync::oneshot::channel();
                let task = tokio::spawn(async move {
                    let outcome = async {
                        let invocation_id = capabilities.begin_control(tool).await?;
                        let mut work = std::pin::pin!(execute(
                            tool,
                            arguments,
                            &schema,
                            capabilities.as_ref(),
                            output.as_ref()
                        ));
                        let outcome = poll_fn(|context| {
                            match std::panic::catch_unwind(AssertUnwindSafe(|| {
                                work.as_mut().poll(context)
                            })) {
                                Ok(result) => result,
                                Err(_) => Poll::Ready(Err(Failure::Capability(
                                    CapabilityFailure::new(CapabilityFailureCode::InternalFailure),
                                ))),
                            }
                        })
                        .await;
                        capabilities
                            .finish_control(
                                invocation_id,
                                tool,
                                outcome.as_ref().err().map(Failure::capability_failure),
                            )
                            .await?;
                        outcome
                    }
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
                match outcome {
                Ok(value) => Ok(ToolOutput::json(value)),
                Err(Failure::Capability(failure)) => {
                    if let Some(code) = fatal_capability_failure(failure.code) {
                        return Err(runtime_tool_failure(&tool_failure, code));
                    }
                    tool_result_json(Err(failure)).map(ToolOutput::json)
                        .map_err(|_| runtime_tool_failure(&tool_failure, AgentDriverFailureCode::InternalFailure))
                }
                Err(Failure::Output(AgentOutputFailure::PolicyRejected { reason, available_methods })) => {
                    Err(ToolExecutionError::invalid_args(serde_json::json!({
                        "accepted": false, "reason": reason, "availableMethods": available_methods,
                    }).to_string()))
                }
                Err(Failure::Output(_)) => Err(runtime_tool_failure(&tool_failure, AgentDriverFailureCode::OutputUnavailable)),
            }
            })
        },
    ))
}

async fn execute(
    tool: AgentControlTool,
    arguments: serde_json::Value,
    schema: &serde_json::Value,
    capabilities: &dyn ModelCapabilityExecutor,
    output: &dyn AgentEventOutput,
) -> Result<serde_json::Value, Failure> {
    match tool {
        AgentControlTool::DelegateTask => {
            let task = arguments::decode(arguments, schema)?;
            Ok(model::task_outcome(&capabilities.delegate(task).await?))
        }
        AgentControlTool::FollowupTask => {
            let request = arguments::decode(arguments, schema)?;
            Ok(model::task_outcome(&capabilities.followup(request).await?))
        }
        AgentControlTool::ProposeStatisticalPlan => {
            let plan = arguments::decode::<StatisticalPlan>(arguments, schema)?;
            let receipt = serde_json::json!({ "accepted": true, "plan": &plan });
            output
                .emit(AgentEvent::PlanProposed { plan })
                .await
                .map_err(Failure::Output)?;
            Ok(receipt)
        }
    }
}
