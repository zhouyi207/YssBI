//! Incremental model summaries with durable, verified prefix checkpoints.
use super::{ContextHook, source::summary_source};
use crate::recovery::STREAM_IDLE;
use futures_util::StreamExt;
use rig_core::completion::{CompletionRequest, Message};
use rig_core::streaming::{Item, StreamEvent};
use yss_harness_contract::*;

const SUMMARY_INSTRUCTIONS: &str = include_str!("summary.md");

impl ContextHook {
    pub(super) async fn summarize(
        &self,
        messages: &[&Message],
    ) -> Result<String, AgentDriverFailure> {
        let source = summary_source(messages.iter().copied())
            .map_err(|_| crate::error::invalid_response())?;
        let (threshold, checkpoint) = {
            let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            (state.threshold, state.checkpoint.clone())
        };
        let checkpoint = checkpoint.filter(|checkpoint| {
            checkpoint.processed_bytes > 0
                && !checkpoint.summary.trim().is_empty()
                && source
                    .get(..checkpoint.processed_bytes)
                    .is_some_and(|prefix| {
                        yss_canonical_hash::content_sha256(prefix.as_bytes())
                            == checkpoint.prefix_hash
                    })
        });
        let mut offset = checkpoint.as_ref().map_or(0, |c| c.processed_bytes);
        let mut summary = checkpoint.map_or_else(String::new, |c| c.summary);
        self.progress(offset, source.len(), None).await?;
        tracing::info!(
            domain = "Application",
            event = "harness_compaction_started",
            source_bytes = source.len(),
            resumed_bytes = offset,
            request_budget_bytes = threshold,
            "Harness context compaction started"
        );
        while offset < source.len() {
            // Reserve space for instructions, accumulated summary and provider framing.
            // A real capacity rejection lowers this request budget in the existing retry path.
            let budget = (threshold * 3 / 4)
                .saturating_sub(summary.len() + SUMMARY_INSTRUCTIONS.len())
                .max(1024);
            let end = source.floor_char_boundary(offset.saturating_add(budget).min(source.len()));
            let prompt = format!(
                "Previous checkpoint:\n{summary}\n\nNext conversation fragment:\n{}",
                &source[offset..end]
            );
            let started = std::time::Instant::now();
            summary = self.summary_request(prompt).await?;
            let checkpoint = ContextCompactionCheckpoint {
                processed_bytes: end,
                prefix_hash: yss_canonical_hash::content_sha256(&source.as_bytes()[..end]),
                summary: summary.clone(),
            };
            self.progress(end, source.len(), Some(checkpoint.clone()))
                .await?;
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .checkpoint = Some(checkpoint);
            tracing::info!(
                domain = "Application",
                event = "harness_compaction_fragment_completed",
                completed_bytes = end,
                total_bytes = source.len(),
                summary_bytes = summary.len(),
                elapsed_ms = started.elapsed().as_millis() as u64,
                "Harness context fragment summarized"
            );
            offset = end;
        }
        Ok(summary)
    }

    async fn progress(
        &self,
        completed_bytes: usize,
        total_bytes: usize,
        checkpoint: Option<ContextCompactionCheckpoint>,
    ) -> Result<(), AgentDriverFailure> {
        self.emit(AgentEvent::ContextCompactionProgress {
            completed_bytes,
            total_bytes,
            checkpoint,
        })
        .await
    }

    async fn summary_request(&self, prompt: String) -> Result<String, AgentDriverFailure> {
        let mut request = CompletionRequest::new(prompt)
            .max_tokens(self.max_output_tokens.map(u64::from))
            .preamble(SUMMARY_INSTRUCTIONS);
        request.temperature = self.temperature;
        request.additional_params = Some(self.additional_parameters.clone());
        let mut stream = self.model.stream(request).map_err(map_failure)?;
        let mut text = String::new();
        let mut pending = String::new();
        let mut reasoning = false;
        let mut deadline = tokio::time::Instant::now() + STREAM_IDLE;
        let mut timer = tokio::time::interval(std::time::Duration::from_millis(40));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let result = async {
            loop {
                let item = tokio::select! {
                    biased;
                    _ = self.cancellation.cancelled() => return Err(crate::error::cancelled()),
                    _ = tokio::time::sleep_until(deadline) => return Err(interrupted()),
                    _ = timer.tick() => {
                        self.flush_compaction(&mut pending, reasoning).await?;
                        continue;
                    }
                    item = stream.next() => item,
                };
                let Some(item) = item else { break };
                deadline = tokio::time::Instant::now() + STREAM_IDLE;
                let (delta, is_reasoning) = match item.map_err(map_failure)? {
                    Item::Event(StreamEvent::Text { text: delta, .. }) => {
                        text.push_str(&delta);
                        (delta, false)
                    }
                    Item::Event(StreamEvent::Reasoning { text, .. }) => (text, true),
                    _ => continue,
                };
                if reasoning != is_reasoning {
                    self.flush_compaction(&mut pending, reasoning).await?;
                    reasoning = is_reasoning;
                }
                pending.push_str(&delta);
                if pending.len() >= 4096 {
                    self.flush_compaction(&mut pending, reasoning).await?;
                }
            }
            Ok(())
        }
        .await;
        // Preserve received output even if this summary is interrupted or retried.
        self.flush_compaction(&mut pending, reasoning).await?;
        result?;
        let response = stream.finish().await.map_err(map_failure)?;
        self.emit(crate::run_options::usage_event(
            response.usage,
            self.context_window,
            ModelCallPurpose::Compaction,
        ))
        .await?;
        if matches!(
            response.finish_reason(),
            Some(rig_core::completion::FinishReason::ContentFilter)
        ) {
            return Err(AgentDriverFailure::new(
                AgentDriverFailureCode::ProviderContentFiltered,
            ));
        }
        if response
            .finish_reason()
            .is_some_and(|reason| reason.truncated_output())
        {
            return Err(AgentDriverFailure::new(
                AgentDriverFailureCode::ProviderOutputTruncated,
            ));
        }
        if text.trim().is_empty() {
            return Err(interrupted());
        }
        Ok(text)
    }

    async fn flush_compaction(
        &self,
        pending: &mut String,
        reasoning: bool,
    ) -> Result<(), AgentDriverFailure> {
        if pending.is_empty() {
            return Ok(());
        }
        self.emit(AgentEvent::ContextCompactionDelta {
            delta: std::mem::take(pending),
            reasoning,
        })
        .await
    }
}

fn interrupted() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::ProviderStreamInterrupted)
}
fn map_failure(error: rig_core::error::ProviderError) -> AgentDriverFailure {
    crate::error::map_prompt_failure(rig_agent::completion::PromptError::CompletionError(error))
}
