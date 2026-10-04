//! Streaming text delivery, cancellation and fatal tool signals.

use crate::error::{cancelled, invalid_response, map_prompt_failure};
use rig_agent::agent::{MultiTurnStreamItem, StreamingError, StreamingResult};
use rig_agent::completion::PromptError;
use rig_agent::streaming::StreamedAssistantContent;
use rig_core::completion::FinishReason;
use std::sync::Arc;
use std::time::Duration;
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentEvent, AgentEventOutput, CancellationToken,
};

use futures_util::StreamExt;

async fn flush_text(
    output: &dyn AgentEventOutput,
    pending: &mut String,
) -> Result<(), AgentDriverFailure> {
    if pending.is_empty() {
        return Ok(());
    }
    output
        .emit(AgentEvent::TextDelta {
            delta: std::mem::take(pending),
        })
        .await
        .map_err(|_| AgentDriverFailure::new(AgentDriverFailureCode::OutputUnavailable))
}

pub(crate) async fn consume_text_stream(
    mut stream: StreamingResult,
    output: Arc<dyn AgentEventOutput>,
    cancellation: CancellationToken,
    mut tool_failures: tokio::sync::watch::Receiver<Option<AgentDriverFailure>>,
    final_response_only: bool,
) -> Result<String, AgentDriverFailure> {
    let mut pending = String::new();
    let mut transcript = String::new();
    let mut final_seen = false;
    let mut final_response = String::new();
    let mut separate_turn = false;
    let mut timer = tokio::time::interval(Duration::from_millis(40));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let result = loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break Err(cancelled()),
            failure = wait_tool_failure(&mut tool_failures) => break Err(failure),
            _ = timer.tick() => flush_text(output.as_ref(), &mut pending).await?,
            item = stream.next() => {
                let Some(item) = item else {
                    break if final_seen { Ok(()) } else {
                        tracing::warn!(
                            domain = "Application", event = "harness_stream_failure",
                            reason = "missing_final_response", received_text_bytes = transcript.len(),
                            "Harness response stream ended without a final response"
                        );
                        Err(AgentDriverFailure::new(AgentDriverFailureCode::ProviderStreamInterrupted))
                    };
                };
                let item = match item {
                    Ok(item) => item,
                    Err(StreamingError::Completion(error)) => break Err(map_prompt_failure(PromptError::CompletionError(error))),
                    Err(StreamingError::Prompt(error)) => break Err(map_prompt_failure(*error)),
                };
                match item {
                    MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(text)) => {
                        if text.text.is_empty() { continue; }
                        let first = transcript.is_empty();
                        if separate_turn && !first {
                            pending.push_str("\n\n");
                            transcript.push_str("\n\n");
                        }
                        separate_turn = false;
                        pending.push_str(&text.text);
                        transcript.push_str(&text.text);
                        if first || pending.len() >= 4096 { flush_text(output.as_ref(), &mut pending).await?; }
                    }
                    MultiTurnStreamItem::CompletionCall(call) => {
                        flush_text(output.as_ref(), &mut pending).await?;
                        let finish_reason = match &call.finish_reason {
                            Some(FinishReason::Stop) => "stop",
                            Some(FinishReason::ToolCalls) => "tool_calls",
                            Some(FinishReason::Length) => "length",
                            Some(FinishReason::ContentFilter) => "content_filter",
                            Some(FinishReason::Other(_)) => "other",
                            None => "unspecified",
                        };
                        tracing::info!(
                            domain = "Application", event = "harness_model_completion",
                            model_call = call.call_index + 1, finish_reason,
                            usage_reported = call.usage.has_values(),
                            input_tokens = call.usage.input_tokens, output_tokens = call.usage.output_tokens,
                            reasoning_tokens = call.usage.reasoning_tokens,
                            "Harness model call completed"
                        );
                        let failure = match call.finish_reason {
                            Some(FinishReason::Length) => Some(AgentDriverFailureCode::ProviderOutputTruncated),
                            Some(FinishReason::ContentFilter) => Some(AgentDriverFailureCode::ProviderContentFiltered),
                            _ => None,
                        };
                        if let Some(code) = failure { break Err(AgentDriverFailure::new(code)); }
                        separate_turn = true;
                    }
                    MultiTurnStreamItem::FinalResponse(response) => {
                        final_seen = true;
                        final_response = response.output;
                    }
                    // No retry hooks are installed: silently retaining rejected text would corrupt the transcript.
                    MultiTurnStreamItem::ModelTurnRetried { .. } => {
                        tracing::warn!(
                            domain = "Application", event = "harness_stream_failure",
                            reason = "unexpected_model_retry",
                            "Harness received an unexpected model retry"
                        );
                        break Err(invalid_response());
                    }
                    _ => {
                        // Rig yields tool calls before executing them on the next poll. Publish
                        // preceding text now so Gateway tool events cannot overtake it.
                        flush_text(output.as_ref(), &mut pending).await?;
                    }
                }
            }
        }
    };
    drop(stream);
    flush_text(output.as_ref(), &mut pending).await?;
    result?;
    Ok(if final_response_only {
        final_response
    } else {
        transcript
    })
}

async fn wait_tool_failure(
    receiver: &mut tokio::sync::watch::Receiver<Option<AgentDriverFailure>>,
) -> AgentDriverFailure {
    loop {
        let failure = receiver.borrow_and_update().clone();
        if let Some(failure) = failure {
            return failure;
        }
        if receiver.changed().await.is_err() {
            return AgentDriverFailure::new(AgentDriverFailureCode::InternalFailure);
        }
    }
}
