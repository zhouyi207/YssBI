//! Working context and stop checks attached to Rig's existing model-call lifecycle.
use crate::messages::PreparedMessages;
use crate::recovery::{ModelActivity, TurnOutput};
use rig_agent::agent::hook::*;
use rig_core::completion::{AssistantContent, Message};
use rig_core::{DynModel, operation::Completion};
use std::sync::{Arc, Mutex};
use yss_harness_contract::*;

const COMPACT_AT_BYTES: usize = 192_000;
mod source;
mod summary;

#[derive(Clone)]
pub(crate) struct SamplingBoundary {
    pub(crate) messages: PreparedMessages,
    pub(crate) text_position: usize,
    pub(crate) completed_calls: usize,
}
struct State {
    checkpoint: Option<ContextCompactionCheckpoint>,
    boundary: SamplingBoundary,
    compacted: bool,
    threshold: usize,
    completed_calls: usize,
    last_feedback: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ContextHook {
    model: DynModel<Completion>,
    output: Arc<TurnOutput>,
    capabilities: Arc<dyn ModelCapabilityExecutor>,
    cancellation: CancellationToken,
    max_output_tokens: Option<u32>,
    temperature: Option<f64>,
    additional_parameters: serde_json::Value,
    state: Arc<Mutex<State>>,
    failure: Arc<Mutex<Option<AgentDriverFailure>>>,
    pub(crate) activity: Arc<ModelActivity>,
}

impl ContextHook {
    pub(crate) fn new(
        model: DynModel<Completion>,
        prepared: PreparedMessages,
        output: Arc<TurnOutput>,
        capabilities: Arc<dyn ModelCapabilityExecutor>,
        cancellation: CancellationToken,
        context_window: Option<u32>,
        max_output_tokens: Option<u32>,
    ) -> Self {
        Self {
            model,
            output,
            capabilities,
            cancellation,
            max_output_tokens,
            temperature: None,
            additional_parameters: serde_json::json!({}),
            state: Arc::new(Mutex::new(State {
                checkpoint: prepared.compaction_checkpoint.clone(),
                boundary: SamplingBoundary {
                    messages: prepared,
                    text_position: 0,
                    completed_calls: 0,
                },
                compacted: false,
                threshold: context_window.map_or(COMPACT_AT_BYTES, |window| {
                    // Serialized UTF-8 bytes are a conservative estimate, not a tokenizer.
                    // Reserve output space and 30% for instructions, tools and framing.
                    // Real capacity errors still reduce this budget through restart().
                    let reserved = max_output_tokens.unwrap_or(window / 8);
                    (window.saturating_sub(reserved) as usize).saturating_mul(7) / 10
                }),
                completed_calls: 0,
                last_feedback: None,
            })),
            failure: Arc::new(Mutex::new(None)),
            activity: Arc::new(ModelActivity::default()),
        }
    }
    pub(crate) fn with_model_parameters(
        mut self,
        temperature: Option<f64>,
        additional_parameters: serde_json::Value,
    ) -> Self {
        self.temperature = temperature;
        self.additional_parameters = additional_parameters;
        self
    }
    pub(crate) fn boundary(&self) -> SamplingBoundary {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .boundary
            .clone()
    }
    pub(crate) fn take_failure(&self) -> Option<AgentDriverFailure> {
        self.failure
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
    pub(crate) fn restart(&self, smaller_context: bool) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if smaller_context {
            state.threshold = (state.threshold / 2).max(4_096);
        }
    }
    pub(crate) fn take_compacted(&self) -> bool {
        std::mem::take(
            &mut self
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .compacted,
        )
    }
    fn fail(&self, error: AgentDriverFailure) -> CompletionCallAction {
        *self.failure.lock().unwrap_or_else(|e| e.into_inner()) = Some(error);
        CompletionCallAction::stop("harness_context_failure")
    }
    async fn emit(&self, event: AgentEvent) -> Result<(), AgentDriverFailure> {
        self.output
            .emit(event)
            .await
            .map_err(|_| AgentDriverFailure::new(AgentDriverFailureCode::OutputUnavailable))
    }
}

impl AgentHook for ContextHook {
    async fn on_completion_call(
        &self,
        _ctx: &HookContext,
        event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        self.activity.finish();
        let threshold = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .threshold;
        let history = event.history.to_vec();
        {
            // Capture this sampling boundary before compaction can fail. Every earlier
            // tool has already settled; a summary failure must not replay those tools.
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.boundary.messages.history = history.clone();
            state.boundary.messages.prompt = event.prompt.clone();
            state.boundary.text_position = self.output.position();
            state.boundary.completed_calls = state.completed_calls;
        }
        let mut all = history.clone();
        all.push(event.prompt.clone());
        let size = match serde_json::to_vec(&all) {
            Ok(bytes) => bytes.len(),
            Err(_) => return self.fail(crate::error::invalid_response()),
        };
        if size >= threshold {
            if let Err(error) = self
                .emit(AgentEvent::RuntimeStatus {
                    phase: AgentRuntimePhase::Compacting,
                    attempt: 0,
                })
                .await
            {
                return self.fail(error);
            }
            let summary = match self.summarize(&all).await {
                Ok(value) => value,
                Err(error) => return self.fail(error),
            };
            let compacted_history = vec![Message::user(format!(
                "[Continuation checkpoint]\n{summary}"
            ))];
            let compacted_prompt = Message::user(
                "Continue the original task from the checkpoint. Use recorded successful tool receipts; do not repeat committed operations. Inspect references for any exact value not preserved in the checkpoint.",
            );
            let compacted_messages: Vec<_> = compacted_history
                .iter()
                .chain(std::iter::once(&compacted_prompt))
                .collect();
            if serde_json::to_vec(&compacted_messages).map_or(true, |bytes| bytes.len() >= size) {
                return self.fail(AgentDriverFailure::new(
                    AgentDriverFailureCode::ContextWindowExceeded,
                ));
            }
            if let Err(error) = self
                .emit(AgentEvent::ContextCompacted {
                    summary: summary.clone(),
                })
                .await
            {
                return self.fail(error);
            }
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.boundary.messages.history = compacted_history;
            state.boundary.messages.prompt = compacted_prompt;
            state.compacted = true;
            state.checkpoint = None;
            // Rig's RequestPatch replaces history but cannot replace the pending tool
            // result prompt. Reopen its runner at a complete checkpoint so no orphan
            // tool results or oversized final result remain in the working context.
            return CompletionCallAction::stop("harness_context_compacted");
        }
        self.activity.touch();
        CompletionCallAction::Continue
    }

    async fn on_model_turn_finished(
        &self,
        _ctx: &HookContext,
        event: ModelTurnFinished<'_>,
    ) -> ModelTurnAction {
        self.activity.finish();
        let incomplete = match event.finish_reason {
            Some(rig_core::completion::FinishReason::Length) => {
                Some(AgentDriverFailureCode::ProviderOutputTruncated)
            }
            Some(rig_core::completion::FinishReason::ContentFilter) => {
                Some(AgentDriverFailureCode::ProviderContentFiltered)
            }
            _ => None,
        };
        if let Some(code) = incomplete {
            *self.failure.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(AgentDriverFailure::new(code));
            return ModelTurnAction::stop("harness_context_incomplete_response");
        }
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .completed_calls += 1;
        if event
            .content
            .iter()
            .any(|part| matches!(part, AssistantContent::ToolCall(_)))
        {
            return ModelTurnAction::Continue;
        }
        let Some(feedback) = self.capabilities.completion_feedback() else {
            return ModelTurnAction::Continue;
        };
        let repeated = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let repeated = state.last_feedback.as_ref() == Some(&feedback);
            state.last_feedback = Some(feedback.clone());
            repeated
        };
        let result = if repeated {
            self.emit(AgentEvent::DeliveryBlocked {
                reason: feedback.clone(),
            })
            .await
        } else {
            self.emit(AgentEvent::RuntimeStatus {
                phase: AgentRuntimePhase::CheckingDelivery,
                attempt: 0,
            })
            .await
        };
        if let Err(error) = result {
            *self.failure.lock().unwrap_or_else(|e| e.into_inner()) = Some(error);
            return ModelTurnAction::stop("harness_output_failure");
        }
        if repeated {
            ModelTurnAction::Continue
        } else {
            ModelTurnAction::retry_with_feedback(feedback)
        }
    }

    async fn on_text_delta(&self, _: &HookContext, _: TextDelta<'_>) -> ObservationAction {
        self.activity.touch();
        ObservationAction::Continue
    }
    async fn on_reasoning_delta(
        &self,
        _: &HookContext,
        _: ReasoningDelta<'_>,
    ) -> ObservationAction {
        self.activity.touch();
        ObservationAction::Continue
    }
    async fn on_tool_call_delta(&self, _: &HookContext, _: ToolCallDelta<'_>) -> ObservationAction {
        self.activity.touch();
        ObservationAction::Continue
    }
}
