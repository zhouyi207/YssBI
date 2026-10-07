//! Provider message mapping and the shared live/replay tool-result encoding.

use crate::error::invalid_response;
use rig_core::completion::Message;
use rig_core::completion::message::{AssistantContent, CallId, ToolCall, ToolFunction, ToolName};
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentMessage, CapabilityFailure,
};

pub(crate) fn tool_result_json(
    outcome: Result<yss_harness_contract::AutomationCapabilityResult, CapabilityFailure>,
) -> Result<serde_json::Value, serde_json::Error> {
    match outcome {
        Ok(result) => yss_harness_contract::model::capability_result(&result),
        Err(failure) => Ok(
            serde_json::json!({"state": "failed", "failure": yss_harness_contract::model::failure(&failure)}),
        ),
    }
}

#[derive(Clone)]
pub(crate) struct PreparedMessages {
    pub(crate) compaction_checkpoint: Option<yss_harness_contract::ContextCompactionCheckpoint>,
    pub(crate) preamble: String,
    pub(crate) history: Vec<Message>,
    pub(crate) prompt: Message,
}

#[derive(Default)]
struct PendingTools {
    calls: Vec<(ToolCall, Option<Message>)>,
    after_results: Vec<Message>,
}

impl PendingTools {
    fn push_call(&mut self, call: ToolCall) -> Result<(), AgentDriverFailure> {
        if self
            .calls
            .iter()
            .any(|(existing, _)| existing.id == call.id)
        {
            return Err(invalid_response());
        }
        self.calls.push((call, None));
        Ok(())
    }

    fn push_assistant(&mut self, message: Message, conversation: &mut Vec<Message>) {
        if self.calls.is_empty() {
            conversation.push(message);
        } else {
            self.after_results.push(message);
        }
    }

    fn resolve(
        &mut self,
        id: &str,
        name: &str,
        result: Message,
        conversation: &mut Vec<Message>,
    ) -> Result<(), AgentDriverFailure> {
        let (call, slot) = self
            .calls
            .iter_mut()
            .find(|(call, _)| call.id.wire() == id)
            .ok_or_else(invalid_response)?;
        if call.function.name != name || slot.is_some() {
            return Err(invalid_response());
        }
        *slot = Some(result);
        if self.calls.iter().any(|(_, result)| result.is_none()) {
            return Ok(());
        }
        // One overlapping group becomes one assistant tool_calls message followed
        // by all matching results. No progress text may split this protocol block.
        let mut content = Vec::with_capacity(self.calls.len());
        let mut results = Vec::with_capacity(self.calls.len());
        for (call, result) in self.calls.drain(..) {
            content.push(AssistantContent::ToolCall(call));
            results.push(result.ok_or_else(invalid_response)?);
        }
        conversation.push(Message::Assistant { id: None, content });
        conversation.extend(results);
        conversation.append(&mut self.after_results);
        Ok(())
    }
}

pub(crate) fn prepare_messages(
    messages: Vec<AgentMessage>,
) -> Result<PreparedMessages, AgentDriverFailure> {
    let mut preamble = Vec::new();
    let mut conversation = Vec::new();
    let mut pending = PendingTools::default();
    let mut compaction_checkpoint = None;
    for message in messages {
        match message {
            AgentMessage::CompactionCheckpoint { checkpoint } => {
                compaction_checkpoint = Some(checkpoint)
            }
            AgentMessage::DelegationCall { run_id, task } => {
                let call = ToolCall::new(
                    CallId::from_wire(run_id.to_string()),
                    ToolFunction::new(
                        ToolName::new("delegate_task").map_err(|_| invalid_response())?,
                        serde_json::to_value(yss_harness_contract::model::AgentTaskInput::from(
                            &task,
                        ))
                        .map_err(|_| invalid_response())?,
                    ),
                );
                pending.push_call(call)?;
            }
            AgentMessage::DelegationResult { outcome } => {
                let id = outcome.run_id.to_string();
                let result = Message::tool_result(
                    CallId::from_wire(id.clone()),
                    ToolName::new("delegate_task").map_err(|_| invalid_response())?,
                    yss_harness_contract::model::task_outcome(&outcome).to_string(),
                );
                pending.resolve(&id, "delegate_task", result, &mut conversation)?;
            }
            AgentMessage::System { content } => preamble.push(content),
            AgentMessage::User { content } => {
                if !pending.calls.is_empty() {
                    return Err(invalid_response());
                }
                conversation.push(Message::user(content));
            }
            AgentMessage::Assistant { content } => {
                pending.push_assistant(Message::assistant(content), &mut conversation);
            }
            AgentMessage::ToolCall {
                invocation_id,
                request,
            } => {
                let name = request.capability_id().as_str().to_owned();
                let arguments = request.model_arguments().map_err(|_| invalid_response())?;
                let call = ToolCall::new(
                    CallId::from_wire(invocation_id.to_string()),
                    ToolFunction::new(
                        ToolName::new(name).map_err(|_| invalid_response())?,
                        arguments,
                    ),
                );
                pending.push_call(call)?;
            }
            AgentMessage::ToolResult {
                invocation_id,
                capability_id,
                outcome,
            } => {
                let result = tool_result_json(outcome).map_err(|_| {
                    AgentDriverFailure::new(AgentDriverFailureCode::InternalFailure)
                })?;
                let id = invocation_id.to_string();
                let result = Message::tool_result(
                    CallId::from_wire(id.clone()),
                    ToolName::new(capability_id.as_str()).map_err(|_| invalid_response())?,
                    result.to_string(),
                );
                pending.resolve(&id, capability_id.as_str(), result, &mut conversation)?;
            }
            AgentMessage::Plan { plan } => {
                pending.push_assistant(
                    Message::assistant(format!(
                        "[Statistical plan]\n{}",
                        serde_json::to_string(&plan).map_err(|_| invalid_response())?
                    )),
                    &mut conversation,
                );
            }
        }
    }
    if !pending.calls.is_empty() {
        return Err(invalid_response());
    }
    let prompt = conversation.pop().ok_or_else(invalid_response)?;
    if !matches!(prompt, Message::User { .. }) {
        return Err(invalid_response());
    }
    Ok(PreparedMessages {
        compaction_checkpoint,
        preamble: preamble.join("\n\n"),
        history: conversation,
        prompt,
    })
}
