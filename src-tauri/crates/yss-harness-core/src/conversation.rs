//! Rebuild model context from the persisted conversation and authoritative tool receipts.

use yss_harness_contract::{
    AgentEvent, AgentMessage, CapabilityId, HarnessEvent, HarnessEventStorePort, HarnessSessionId,
    HarnessTurnId, PersistenceFailure, PersistenceFailureCode, ToolInvocationId,
    ToolInvocationLedgerPort, ToolInvocationRecord, ToolInvocationState,
};

pub(crate) async fn history(
    events: &dyn HarnessEventStorePort,
    ledger: &dyn ToolInvocationLedgerPort,
    session: &HarnessSessionId,
    current_turn: &HarnessTurnId,
) -> Result<Vec<AgentMessage>, PersistenceFailure> {
    let mut messages = Vec::new();
    let mut pending = Vec::<ToolInvocationRecord>::new();
    let mut has_text = false;
    let mut sequence = 0;
    for envelope in events.load_events_after(session, 0).await? {
        if &envelope.session_id != session || envelope.sequence != sequence + 1 {
            return Err(invalid_record());
        }
        sequence = envelope.sequence;
        // This turn's user message is appended by the prompt builder exactly once.
        if envelope.turn_id.as_ref() == Some(current_turn) {
            finish_pending(&mut messages, &mut pending)?;
            return Ok(messages);
        }
        match envelope.event {
            HarnessEvent::TurnStarted { user_message } => {
                finish_pending(&mut messages, &mut pending)?;
                messages.push(AgentMessage::User {
                    content: user_message,
                });
                has_text = false;
            }
            HarnessEvent::Agent(AgentEvent::TextDelta { delta }) => {
                has_text |= !delta.is_empty();
                assistant_text(&mut messages, delta);
            }
            HarnessEvent::Agent(AgentEvent::ToolInvocationStarted {
                invocation_id,
                capability_id,
            }) => {
                let record = invocation(
                    ledger,
                    session,
                    envelope.turn_id.as_ref(),
                    &invocation_id,
                    capability_id,
                )
                .await?;
                messages.push(tool_call(&record));
                pending.push(record);
            }
            HarnessEvent::Agent(AgentEvent::ToolInvocationCompleted {
                invocation_id,
                capability_id,
            })
            | HarnessEvent::Agent(AgentEvent::ToolInvocationFailed {
                invocation_id,
                capability_id,
                ..
            }) => {
                let record = if let Some(index) =
                    pending.iter().position(|record| record.id == invocation_id)
                {
                    pending.remove(index)
                } else {
                    // A call rejected before admission, or recovered at startup, can lack a started event.
                    let record = invocation(
                        ledger,
                        session,
                        envelope.turn_id.as_ref(),
                        &invocation_id,
                        capability_id,
                    )
                    .await?;
                    messages.push(tool_call(&record));
                    record
                };
                if record.capability_id != capability_id
                    || Some(&record.turn_id) != envelope.turn_id.as_ref()
                {
                    return Err(invalid_record());
                }
                messages.push(tool_result(record)?);
            }
            HarnessEvent::Agent(AgentEvent::PlanProposed { plan }) => {
                messages.push(AgentMessage::Plan { plan });
            }
            HarnessEvent::TurnCompleted { final_text } => {
                finish_pending(&mut messages, &mut pending)?;
                // Streaming text already contains the full public reply, including progress text.
                if !has_text {
                    assistant_text(&mut messages, final_text);
                }
            }
            HarnessEvent::TurnFailed | HarnessEvent::TurnCancelled => {
                let status = if matches!(envelope.event, HarnessEvent::TurnCancelled) {
                    "cancelled"
                } else {
                    "failed"
                };
                finish_pending(&mut messages, &mut pending)?;
                assistant_text(
                    &mut messages,
                    format!(
                        "\n[Harness turn status: {status}. Recorded tool outcomes remain authoritative; do not assume completed operations were rolled back.]"
                    ),
                );
            }
            _ => {}
        }
    }
    // Never silently send a partial conversation when its current-turn boundary is missing.
    Err(invalid_record())
}

fn assistant_text(messages: &mut Vec<AgentMessage>, content: String) {
    if content.is_empty() {
        return;
    }
    if let Some(AgentMessage::Assistant { content: previous }) = messages.last_mut() {
        previous.push_str(&content);
    } else {
        messages.push(AgentMessage::Assistant { content });
    }
}

async fn invocation(
    ledger: &dyn ToolInvocationLedgerPort,
    session: &HarnessSessionId,
    turn: Option<&HarnessTurnId>,
    id: &ToolInvocationId,
    capability: CapabilityId,
) -> Result<ToolInvocationRecord, PersistenceFailure> {
    let record = ledger
        .load_invocation(session, id)
        .await?
        .ok_or_else(invalid_record)?;
    if &record.session_id != session
        || Some(&record.turn_id) != turn
        || record.capability_id != capability
    {
        return Err(invalid_record());
    }
    Ok(record)
}

fn tool_call(record: &ToolInvocationRecord) -> AgentMessage {
    AgentMessage::ToolCall {
        invocation_id: record.id.clone(),
        request: record.request.clone(),
    }
}

fn tool_result(record: ToolInvocationRecord) -> Result<AgentMessage, PersistenceFailure> {
    let outcome = match record.state {
        ToolInvocationState::Succeeded => Ok(record.result.ok_or_else(invalid_record)?),
        ToolInvocationState::Failed => Err(record.failure.ok_or_else(invalid_record)?),
        ToolInvocationState::Running => Err(yss_harness_contract::CapabilityFailure::new(
            yss_harness_contract::CapabilityFailureCode::OutcomeUnknown,
        )),
    };
    Ok(AgentMessage::ToolResult {
        invocation_id: record.id,
        capability_id: record.capability_id,
        outcome,
    })
}

fn finish_pending(
    messages: &mut Vec<AgentMessage>,
    pending: &mut Vec<ToolInvocationRecord>,
) -> Result<(), PersistenceFailure> {
    for record in pending.drain(..) {
        messages.push(tool_result(record)?);
    }
    Ok(())
}

fn invalid_record() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::InvalidRecord)
}
