//! Ordered output shared by the manager, workers and context-summary calls.
use super::Tool;
use crate::assistant::activity::Activity;
use yss_harness_contract::{
    AgentRuntimePhase, AssistantEventKind as Event, AssistantToolIdentity, KnowledgeCitation,
    ModelCallPurpose, ModelTokenUsage, StatisticalPlan,
};

#[derive(Clone)]
pub(in crate::assistant) struct Text {
    pub text: String,
    // UTF-8 byte boundary: retried text remains inspectable but isn't copied as the reply.
    pub valid_bytes: usize,
}
#[derive(Clone)]
pub(in crate::assistant) struct Usage {
    pub tokens: ModelTokenUsage,
    pub context_window: Option<u32>,
    pub purpose: ModelCallPurpose,
}
#[derive(Clone, Default)]
pub(in crate::assistant) struct Compaction {
    pub output: Output,
    pub completed_bytes: usize,
    pub total_bytes: usize,
    pub finished: bool,
    pub interrupted: bool,
}
#[derive(Clone)]
pub(in crate::assistant) enum Content {
    Text(Text),
    Reasoning(String),
    Tool(Tool),
    Task(usize),
    Outcome(usize),
    Plan(Box<StatisticalPlan>),
    Citation(KnowledgeCitation),
    Failure(String),
    Status { event: Box<Event>, error: bool },
    Usage(Usage),
    Compaction(Box<Compaction>),
}
#[derive(Clone)]
pub(in crate::assistant) struct Part {
    pub id: u64,
    pub content: Content,
}
#[derive(Clone, Default)]
pub(in crate::assistant) struct Output {
    pub parts: Vec<Part>,
    pub activity: Option<Activity>,
}
impl Output {
    pub fn finish_timing(&mut self, at: u64, cancelled: bool) {
        for part in &mut self.parts {
            match &mut part.content {
                Content::Tool(tool) => tool.settle(at, cancelled),
                Content::Compaction(compaction) if !compaction.finished => {
                    compaction.interrupted = true;
                    compaction.output.finish_timing(at, cancelled);
                }
                _ => {}
            }
        }
    }
    pub fn push(&mut self, id: u64, content: Content) {
        self.parts.push(Part { id, content });
    }
    fn valid_text(&self) -> impl DoubleEndedIterator<Item = &str> {
        self.parts.iter().filter_map(|part| match &part.content {
            Content::Text(text) => Some(&text.text[..text.valid_bytes]),
            _ => None,
        })
    }
    pub fn has_text(&self) -> bool {
        self.valid_text().any(|text| !text.is_empty())
    }
    pub fn text(&self) -> String {
        self.valid_text().collect()
    }
    pub fn ends_with(&self, suffix: &str) -> bool {
        let mut suffix = suffix.as_bytes();
        for text in self.valid_text().rev() {
            let text = text.as_bytes();
            let count = text.len().min(suffix.len());
            if text[text.len() - count..] != suffix[suffix.len() - count..] {
                return false;
            }
            suffix = &suffix[..suffix.len() - count];
            if suffix.is_empty() {
                return true;
            }
        }
        suffix.is_empty()
    }
    pub fn failure(&self) -> Option<&str> {
        self.parts
            .iter()
            .rev()
            .find_map(|part| match &part.content {
                Content::Failure(code) => Some(code.as_str()),
                _ => None,
            })
    }
    pub fn fail(&mut self, id: u64, code: String) {
        if let Some(previous) = self
            .parts
            .iter_mut()
            .find_map(|part| match &mut part.content {
                Content::Failure(code) => Some(code),
                _ => None,
            })
        {
            *previous = code;
        } else {
            self.push(id, Content::Failure(code));
        }
    }
    pub fn complete_text(&mut self, id: u64, text: String) {
        // Core and React treat completion as a fallback, never a second streamed reply.
        if !text.is_empty() && !self.has_text() {
            self.push(
                id,
                Content::Text(Text {
                    valid_bytes: text.len(),
                    text,
                }),
            );
        }
    }
    pub fn accept(&mut self, id: u64, at: u64, event: Event) -> Result<(), ()> {
        Activity::accept(&mut self.activity, &event);
        let tool_finished = matches!(
            &event,
            Event::ToolInvocationCompleted { .. }
                | Event::ToolInvocationFailed { .. }
                | Event::GraphExecutionFinished { .. }
        );
        match event {
            Event::TextDelta { delta } => self.delta(id, delta, false),
            Event::ReasoningDelta { delta } => self.delta(id, delta, true),
            Event::TextRetracted { characters } => self.retract(characters),
            Event::UsageReported {
                usage,
                context_window,
                purpose,
            } => {
                let usage = Content::Usage(Usage {
                    tokens: usage,
                    context_window,
                    purpose,
                });
                if purpose == ModelCallPurpose::Compaction {
                    self.compaction(id).output.push(id, usage);
                } else {
                    self.push(id, usage);
                }
            }
            Event::KnowledgeCited { citation } => self.push(id, Content::Citation(citation)),
            Event::PlanProposed { plan } => self.push(id, Content::Plan(Box::new(plan))),
            Event::RuntimeStatus { phase, attempt } => {
                if matches!(
                    phase,
                    AgentRuntimePhase::Reconnecting | AgentRuntimePhase::Compacting
                ) {
                    for part in &mut self.parts {
                        if let Content::Compaction(value) = &mut part.content
                            && !value.finished
                        {
                            value.interrupted = true;
                        }
                    }
                }
                let event = Event::RuntimeStatus { phase, attempt };
                self.status(id, event, false);
            }
            Event::ContextCompactionProgress {
                completed_bytes,
                total_bytes,
            } => {
                let compaction = self.compaction(id);
                compaction.completed_bytes = completed_bytes;
                compaction.total_bytes = total_bytes;
            }
            Event::ContextCompactionDelta { delta, reasoning } => {
                self.compaction(id).output.delta(id, delta, reasoning)
            }
            Event::ContextCompacted { summary } => {
                let compaction = self.compaction(id);
                compaction.output.complete_text(id, summary);
                compaction.finished = true;
                self.activity = None;
            }
            Event::DeliveryBlocked { reason } => {
                self.fail(id, reason);
            }
            Event::ToolInvocationStarted {
                invocation_id,
                capability_id,
            } => self.push(
                id,
                Content::Tool(Tool::started(invocation_id, capability_id, at)),
            ),
            Event::ToolInvocationCompleted {
                invocation_id,
                capability_id,
            } => {
                self.tool(id, invocation_id, capability_id).completed(at);
            }
            Event::ToolInvocationFailed {
                invocation_id,
                failure_code,
                capability_id,
            } => {
                self.tool(id, invocation_id, capability_id)
                    .failed(at, failure_code);
            }
            Event::GraphExecutionFinished {
                invocation_id,
                status,
                failure_code,
            } => {
                self.tool(
                    id,
                    invocation_id,
                    yss_harness_contract::CapabilityId::ExecuteGraph.into(),
                )
                .executed(at, &status, failure_code);
            }
            event @ (Event::WorkflowPlanned { .. }
            | Event::WorkflowStarted { .. }
            | Event::WorkflowStepStarted { .. }
            | Event::WorkflowStepCompleted { .. }
            | Event::WorkflowCompleted { .. }
            | Event::WorkflowPaused { .. }
            | Event::WorkflowResumed { .. }
            | Event::WorkflowCancelled { .. }) => self.status(id, event, false),
            event @ Event::WorkflowStepFailed { .. } => self.status(id, event, true),
            // Lifecycle events belong to Transcript/Turn, never inside an agent's output.
            Event::SessionCreated
            | Event::TurnStarted { .. }
            | Event::TurnConfigured { .. }
            | Event::TurnCompleted { .. }
            | Event::TurnFailed
            | Event::TurnCancelled
            | Event::AgentRunStarted { .. }
            | Event::AgentRunResumed { .. }
            | Event::AgentRunOutput { .. }
            | Event::AgentRunFinished { .. }
            | Event::AgentRunInvalidated { .. } => return Err(()),
        }
        if tool_finished {
            self.activity = self
                .parts
                .iter()
                .rev()
                .find_map(|part| match &part.content {
                    Content::Tool(tool) if tool.running() => Some(Activity::Tool(tool.kind)),
                    _ => None,
                });
        }
        Ok(())
    }
    fn delta(&mut self, id: u64, delta: String, reasoning: bool) {
        if delta.is_empty() {
            return;
        }
        match (
            self.parts.last_mut().map(|part| &mut part.content),
            reasoning,
        ) {
            (Some(Content::Reasoning(text)), true) => text.push_str(&delta),
            (Some(Content::Text(text)), false) if text.valid_bytes == text.text.len() => {
                text.text.push_str(&delta);
                text.valid_bytes = text.text.len();
            }
            (_, true) => self.push(id, Content::Reasoning(delta)),
            (_, false) => self.push(
                id,
                Content::Text(Text {
                    valid_bytes: delta.len(),
                    text: delta,
                }),
            ),
        }
    }
    fn retract(&mut self, mut characters: usize) {
        for part in self.parts.iter_mut().rev() {
            let Content::Text(text) = &mut part.content else {
                continue;
            };
            let active = &text.text[..text.valid_bytes];
            let count = active.chars().count();
            let keep = count.saturating_sub(characters);
            text.valid_bytes = active
                .char_indices()
                .nth(keep)
                .map_or(active.len(), |(i, _)| i);
            characters = characters.saturating_sub(count);
            if characters == 0 {
                break;
            }
        }
    }
    fn tool(&mut self, sequence: u64, id: String, kind: AssistantToolIdentity) -> &mut Tool {
        let index = self
            .parts
            .iter()
            .rposition(|part| matches!(&part.content, Content::Tool(tool) if tool.id == id))
            .unwrap_or_else(|| {
                // Recovery can close a ledger entry whose start event wasn't published.
                self.push(sequence, Content::Tool(Tool::recovered(id, kind)));
                self.parts.len() - 1
            });
        let Content::Tool(tool) = &mut self.parts[index].content else {
            unreachable!()
        };
        tool
    }
    fn compaction(&mut self, id: u64) -> &mut Compaction {
        let existing = self.parts.iter().rposition(
            |part| matches!(&part.content, Content::Compaction(value) if !value.finished && !value.interrupted),
        );
        let index = existing.unwrap_or_else(|| {
            self.push(id, Content::Compaction(Box::default()));
            self.parts.len() - 1
        });
        let Content::Compaction(value) = &mut self.parts[index].content else {
            unreachable!()
        };
        value
    }
    fn status(&mut self, id: u64, event: Event, error: bool) {
        // Keep the native event so changing language can redraw history without replaying it.
        self.push(
            id,
            Content::Status {
                event: Box::new(event),
                error,
            },
        );
    }
}
