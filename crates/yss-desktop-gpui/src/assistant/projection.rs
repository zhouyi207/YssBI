//! Read-only transcript reduced from the existing public event projection.
use std::collections::BTreeMap;
use yss_harness_contract::{
    AgentRole, AgentRunState, HarnessResourceReference, HarnessTurnOptions, KnowledgeCitation,
    LanguageModelIdentity, ModelCallPurpose, ModelTokenUsage, StatisticalPlan,
};
use yss_harness_contract::{
    AssistantEvent, AssistantEventKind as Event, AssistantResultReference, AssistantToolIdentity,
};

#[derive(Clone, Copy)]
pub(super) struct Timing {
    pub started_at: u64,
    pub updated_at: u64,
    pub finished_at: Option<u64>,
}
impl Timing {
    fn started(at: u64) -> Self {
        Self {
            started_at: at,
            updated_at: at,
            finished_at: None,
        }
    }
    fn finish(&mut self, at: u64) {
        if self.finished_at.is_none() {
            self.updated_at = at;
            self.finished_at = Some(at);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum TurnState {
    Running,
    Completed,
    Failed,
    Cancelled,
}
#[derive(Clone)]
pub(super) struct Tool {
    pub id: String,
    pub kind: AssistantToolIdentity,
    pub finished: bool,
    pub failure: Option<String>,
    pub execution: Option<String>,
    pub timing: Option<Timing>,
}
#[derive(Clone)]
pub(super) struct Task {
    pub id: String,
    pub role: AgentRole,
    pub objective: String,
    pub timing: Timing,
    pub state: Option<AgentRunState>,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub artifacts: Vec<yss_harness_contract::ResourceChange>,
    pub results: Vec<AssistantResultReference>,
    pub tools: Vec<Tool>,
}
#[derive(Clone)]
pub(super) struct Turn {
    pub id: String,
    pub user: String,
    pub model: LanguageModelIdentity,
    pub timing: Timing,
    pub resources: Vec<HarnessResourceReference>,
    pub text: String,
    pub reasoning: String,
    pub tools: Vec<Tool>,
    pub tasks: BTreeMap<String, Task>,
    pub citations: Vec<KnowledgeCitation>,
    pub plan: Option<StatisticalPlan>,
    pub options: Option<HarnessTurnOptions>,
    pub usage: Vec<(ModelTokenUsage, Option<u32>, ModelCallPurpose)>,
    pub activity: Option<String>,
    pub error: Option<String>,
    pub state: TurnState,
}
#[derive(Clone, Default)]
pub(super) struct Transcript {
    pub sequence: u64,
    pub turns: Vec<Turn>,
}
impl Transcript {
    pub fn running(&self) -> bool {
        self.turns
            .last()
            .is_some_and(|turn| turn.state == TurnState::Running)
    }
    pub fn accept(&mut self, event: AssistantEvent) -> Result<(), ()> {
        if event.sequence <= self.sequence {
            return Ok(());
        }
        if event.sequence != self.sequence + 1 {
            return Err(());
        }
        if let Event::TurnStarted {
            user_message,
            model,
            resources,
        } = event.event
        {
            self.turns.push(Turn {
                id: event.turn_id.ok_or(())?,
                user: user_message,
                model,
                timing: Timing::started(event.occurred_at),
                resources,
                text: String::new(),
                reasoning: String::new(),
                tools: vec![],
                tasks: BTreeMap::new(),
                citations: vec![],
                plan: None,
                options: None,
                usage: vec![],
                activity: None,
                error: None,
                state: TurnState::Running,
            });
        } else if let Some(id) = event.turn_id {
            let Some(turn) = self.turns.iter_mut().rev().find(|turn| turn.id == id) else {
                return Err(());
            };
            turn.accept(event.occurred_at, event.event);
        }
        self.sequence = event.sequence;
        Ok(())
    }
}
impl Turn {
    fn finish_timing(&mut self, at: u64) {
        self.timing.finish(at);
        finish_tool_timings(&mut self.tools, at);
        for task in self.tasks.values_mut().filter(|task| task.state.is_none()) {
            task.timing.finish(at);
            finish_tool_timings(&mut task.tools, at);
        }
    }
    fn accept(&mut self, at: u64, event: Event) {
        self.timing.updated_at = at;
        match event {
            Event::TurnConfigured { options } => self.options = Some(options),
            Event::TextDelta { delta } => self.text.push_str(&delta),
            Event::TextRetracted { characters } => {
                let count = self.text.chars().count().saturating_sub(characters);
                let end = self
                    .text
                    .char_indices()
                    .nth(count)
                    .map(|(i, _)| i)
                    .unwrap_or(self.text.len());
                self.text.truncate(end);
            }
            Event::ReasoningDelta { delta } => self.reasoning.push_str(&delta),
            Event::UsageReported {
                usage,
                context_window,
                purpose,
            } => self.usage.push((usage, context_window, purpose)),
            Event::TurnCompleted { final_text } => {
                self.text = final_text;
                self.state = TurnState::Completed;
                self.finish_timing(at);
                self.activity = None;
            }
            Event::TurnFailed => {
                self.state = TurnState::Failed;
                self.finish_timing(at);
                self.activity = None;
            }
            Event::TurnCancelled => {
                self.state = TurnState::Cancelled;
                self.finish_timing(at);
                self.activity = None;
            }
            Event::KnowledgeCited { citation } => {
                if !self.citations.contains(&citation) {
                    self.citations.push(citation);
                }
            }
            Event::PlanProposed { plan } => self.plan = Some(plan),
            Event::RuntimeStatus { phase, attempt } => {
                self.activity = Some(format!("{} · {}", phase_label(phase), attempt))
            }
            Event::ContextCompactionProgress {
                completed_bytes,
                total_bytes,
            } => self.activity = Some(format!("压缩上下文 · {completed_bytes}/{total_bytes}")),
            Event::ContextCompacted { .. } => self.activity = None,
            Event::DeliveryBlocked { reason } => self.error = Some(reason),
            Event::AgentRunStarted {
                run_id,
                role,
                objective,
                ..
            }
            | Event::AgentRunResumed {
                run_id,
                role,
                objective,
            } => {
                self.tasks.insert(
                    run_id.clone(),
                    Task {
                        id: run_id,
                        role,
                        objective,
                        timing: Timing::started(at),
                        state: None,
                        summary: None,
                        error: None,
                        warnings: vec![],
                        artifacts: vec![],
                        results: vec![],
                        tools: vec![],
                    },
                );
            }
            Event::AgentRunOutput { run_id, event } => {
                if self
                    .tasks
                    .get(&run_id)
                    .is_some_and(|task| task.role == AgentRole::Manager)
                {
                    self.accept(at, *event);
                } else if let Some(task) = self.tasks.get_mut(&run_id) {
                    task.timing.updated_at = at;
                    apply_tool(&mut task.tools, at, *event);
                }
            }
            Event::AgentRunFinished {
                run_id,
                role,
                state,
                failure_code,
                summary,
                blocked_reason,
                warnings,
                artifacts,
                results,
                ..
            } => {
                if let Some(task) = self.tasks.get_mut(&run_id) {
                    task.timing.finish(at);
                    finish_tool_timings(&mut task.tools, at);
                    task.state = Some(state);
                    task.summary = summary;
                    task.error = failure_code.map(|code| code.to_string()).or(blocked_reason);
                    task.warnings = warnings;
                    task.artifacts = artifacts;
                    task.results = results;
                    if role == AgentRole::Manager && task.error.is_some() {
                        self.error = task.error.clone();
                    }
                }
            }
            Event::AgentRunInvalidated { run_id } => {
                if let Some(task) = self.tasks.get_mut(&run_id) {
                    task.timing.updated_at = at;
                    task.state = Some(AgentRunState::Stale);
                }
            }
            event => apply_tool(&mut self.tools, at, event),
        }
    }
}
fn finish_tool_timings(tools: &mut [Tool], at: u64) {
    for tool in tools {
        if let Some(timing) = &mut tool.timing {
            timing.finish(at);
        }
    }
}
fn apply_tool(tools: &mut Vec<Tool>, at: u64, event: Event) {
    match event {
        Event::ToolInvocationStarted {
            invocation_id,
            capability_id,
        } => tools.push(Tool {
            id: invocation_id,
            kind: capability_id,
            finished: false,
            failure: None,
            execution: None,
            timing: Some(Timing::started(at)),
        }),
        Event::ToolInvocationCompleted { invocation_id, .. } => {
            if let Some(tool) = tools.iter_mut().find(|tool| tool.id == invocation_id) {
                if let Some(timing) = &mut tool.timing {
                    timing.finish(at);
                }
                tool.finished = true;
            }
        }
        Event::ToolInvocationFailed {
            invocation_id,
            failure_code,
            ..
        } => {
            if let Some(tool) = tools.iter_mut().find(|tool| tool.id == invocation_id) {
                if let Some(timing) = &mut tool.timing {
                    timing.finish(at);
                }
                tool.finished = true;
                tool.failure = Some(failure_code);
            }
        }
        Event::GraphExecutionFinished {
            invocation_id,
            status,
            failure_code,
        } => {
            if let Some(tool) = tools.iter_mut().find(|tool| tool.id == invocation_id) {
                tool.execution = Some(status);
                tool.failure = failure_code.or(tool.failure.take());
            }
        }
        _ => {}
    }
}
fn phase_label(phase: yss_harness_contract::AgentRuntimePhase) -> &'static str {
    match phase {
        yss_harness_contract::AgentRuntimePhase::CheckingDelivery => "检查交付",
        yss_harness_contract::AgentRuntimePhase::Reconnecting => "重新连接",
        yss_harness_contract::AgentRuntimePhase::Compacting => "压缩上下文",
    }
}
