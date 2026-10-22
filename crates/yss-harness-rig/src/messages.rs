//! Provider message mapping for replayed Harness history.

use crate::error::invalid_response;
use rig_core::completion::message::{AssistantContent, CallId, ToolCall, ToolFunction, ToolName};
use rig_core::completion::{AssistantMessage, Message};
use yss_harness_contract::{AgentDriverFailure, AgentMessage};

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
        conversation.push(Message::Assistant(AssistantMessage::new(content)));
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
                tool,
                arguments,
            } => {
                let call = ToolCall::new(
                    CallId::from_wire(invocation_id),
                    ToolFunction::new(
                        ToolName::new(tool).map_err(|_| invalid_response())?,
                        arguments,
                    ),
                );
                pending.push_call(call)?;
            }
            AgentMessage::ToolResult {
                invocation_id,
                tool,
                output,
            } => {
                let result = Message::tool_result(
                    CallId::from_wire(invocation_id.clone()),
                    ToolName::new(tool.clone()).map_err(|_| invalid_response())?,
                    output.to_string(),
                );
                pending.resolve(&invocation_id, &tool, result, &mut conversation)?;
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
