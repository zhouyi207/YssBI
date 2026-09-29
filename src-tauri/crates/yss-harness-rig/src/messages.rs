//! Provider message mapping and the shared live/replay tool-result encoding.

use crate::error::invalid_response;
use rig_core::completion::Message;
use rig_core::completion::message::{AssistantContent, ToolCall, ToolCallId, ToolFunction};
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentMessage, CapabilityFailure,
};

pub(crate) fn tool_result_json(
    outcome: Result<yss_harness_contract::AutomationCapabilityResult, CapabilityFailure>,
) -> Result<serde_json::Value, serde_json::Error> {
    match outcome {
        Ok(result) => serde_json::to_value(result),
        Err(failure) => serde_json::to_value(failure)
            .map(|failure| serde_json::json!({"state": "failed", "failure": failure})),
    }
}

pub(crate) struct PreparedMessages {
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
            .any(|(existing, _)| existing.wire_call_id() == call.wire_call_id())
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
            .find(|(call, _)| call.wire_call_id() == id)
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
    for message in messages {
        match message {
            AgentMessage::DelegationCall { run_id, task } => {
                let call = ToolCall::new(
                    ToolCallId::new(run_id.to_string()).ok_or_else(invalid_response)?,
                    ToolFunction::new(
                        "delegate_task".to_owned(),
                        serde_json::to_value(task).map_err(|_| invalid_response())?,
                    ),
                );
                pending.push_call(call)?;
            }
            AgentMessage::DelegationResult { outcome } => {
                let id = outcome.run_id.to_string();
                let result = Message::tool_result(
                    id.clone(),
                    "delegate_task",
                    serde_json::to_string(&outcome).map_err(|_| invalid_response())?,
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
                let mut encoded = serde_json::to_value(request).map_err(|_| invalid_response())?;
                let arguments = encoded
                    .get_mut("payload")
                    .ok_or_else(invalid_response)?
                    .take();
                let call = ToolCall::new(
                    ToolCallId::new(invocation_id.to_string()).ok_or_else(invalid_response)?,
                    ToolFunction::new(name, arguments),
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
                let result =
                    Message::tool_result(id.clone(), capability_id.as_str(), result.to_string());
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
        preamble: preamble.join("\n\n"),
        history: conversation,
        prompt,
    })
}
