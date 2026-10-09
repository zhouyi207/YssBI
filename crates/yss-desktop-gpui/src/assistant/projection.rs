//! Read-only transcript reduced identically from live and persisted events.
mod output;
mod tool;
pub(super) mod usage;
pub(super) use output::{Compaction, Content, Output, Part, Usage};
pub(super) use tool::{Tool, ToolState};
use yss_harness_contract::{
    AgentRole, AgentRunState, AssistantEvent, AssistantEventKind as Event,
    AssistantResultReference, HarnessResourceReference, HarnessTurnOptions, LanguageModelIdentity,
};

#[derive(Clone, Copy, PartialEq)]
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
pub(super) struct Task {
    pub id: String,
    pub role: AgentRole,
    pub objective: String,
    pub timing: Timing,
    pub state: Option<AgentRunState>,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub blocked_reason: Option<String>,
    pub warnings: Vec<String>,
    pub artifacts: Vec<yss_harness_contract::ResourceChange>,
    pub results: Vec<AssistantResultReference>,
    pub output: Output,
}

impl Task {
    fn warn(&mut self, warning: String) {
        if !self.warnings.contains(&warning) {
            self.warnings.push(warning);
        }
    }
    fn execution_warning(&mut self, event: &Event) {
        if let Event::GraphExecutionFinished {
            status,
            failure_code,
            ..
        } = event
            && status != "succeeded"
        {
            self.warn(failure_code.as_ref().unwrap_or(status).clone());
        }
    }
    pub fn activity(&self) -> Option<&super::activity::Activity> {
        self.output.activity.as_ref()
    }
    pub fn failure(&self) -> Option<&str> {
        self.error.as_deref().or_else(|| self.output.failure())
    }
}

#[derive(Clone)]
pub(super) struct Turn {
    pub id: String,
    pub user: String,
    pub model: LanguageModelIdentity,
    pub timing: Timing,
    pub resources: Vec<HarnessResourceReference>,
    pub consumption: usage::TurnUsage,
    pub output: Output,
    // Each resume has its own entry so its earlier output is never overwritten.
    pub tasks: Vec<Task>,
    pub options: Option<HarnessTurnOptions>,
    pub state: TurnState,
}

#[derive(Clone, Default)]
pub(super) struct Transcript {
    pub sequence: u64,
    pub turns: Vec<Turn>,
    pub session_output: Output,
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
        match event.event {
            Event::SessionCreated => {}
            Event::TurnStarted {
                user_message,
                model,
                resources,
            } => self.turns.push(Turn {
                id: event.turn_id.ok_or(())?,
                user: user_message,
                model,
                timing: Timing::started(event.occurred_at),
                resources,
                consumption: Default::default(),
                output: Output::default(),
                tasks: vec![],
                options: None,
                state: TurnState::Running,
            }),
            value => {
                if let Some(id) = event.turn_id {
                    let turn = self
                        .turns
                        .iter_mut()
                        .rev()
                        .find(|turn| turn.id == id)
                        .ok_or(())?;
                    turn.accept(event.sequence, event.occurred_at, value)?;
                } else {
                    self.session_output
                        .accept(event.sequence, event.occurred_at, value)?;
                }
            }
        }
        self.sequence = event.sequence;
        Ok(())
    }
}
impl Turn {
    fn finish_timing(&mut self, at: u64) {
        self.timing.finish(at);
        self.output
            .finish_timing(at, self.state == TurnState::Cancelled);
        for task in &mut self.tasks {
            if task.state.is_none() {
                task.timing.finish(at);
                task.output
                    .finish_timing(at, self.state == TurnState::Cancelled);
                task.state = Some(if self.state == TurnState::Cancelled {
                    AgentRunState::Cancelled
                } else {
                    AgentRunState::Interrupted
                });
                task.output.activity = None;
            }
        }
    }
    fn accept(&mut self, sequence: u64, at: u64, event: Event) -> Result<(), ()> {
        self.timing.updated_at = at;
        self.consumption.accept(&event, false);
        match event {
            Event::TurnConfigured { options } => self.options = Some(options),
            Event::TurnCompleted { final_text } => {
                self.output.complete_text(sequence, final_text);
                self.state = TurnState::Completed;
                self.finish_timing(at);
                self.output.activity = None;
            }
            Event::TurnFailed => {
                self.state = TurnState::Failed;
                self.finish_timing(at);
                self.output.activity = None;
            }
            Event::TurnCancelled => {
                self.state = TurnState::Cancelled;
                self.finish_timing(at);
                self.output.activity = None;
            }
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
                if role != AgentRole::Manager {
                    self.output.push(sequence, Content::Task(self.tasks.len()));
                }
                self.tasks.push(Task {
                    id: run_id,
                    role,
                    objective,
                    timing: Timing::started(at),
                    state: None,
                    summary: None,
                    error: None,
                    blocked_reason: None,
                    warnings: vec![],
                    artifacts: vec![],
                    results: vec![],
                    output: Output::default(),
                });
            }
            Event::AgentRunOutput { run_id, event } => {
                let task = self
                    .tasks
                    .iter_mut()
                    .rev()
                    .find(|task| task.id == run_id)
                    .ok_or(())?;
                task.timing.updated_at = at;
                self.consumption
                    .accept(&event, task.role != AgentRole::Manager);
                if task.role == AgentRole::Manager {
                    self.output.accept(sequence, at, *event)?;
                } else {
                    task.execution_warning(&event);
                    task.output.accept(sequence, at, *event)?;
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
                let task = self
                    .tasks
                    .iter_mut()
                    .enumerate()
                    .rev()
                    .find(|(_, task)| task.id == run_id)
                    .ok_or(())?;
                let (index, task) = task;
                task.timing.finish(at);
                task.output
                    .finish_timing(at, state == AgentRunState::Cancelled);
                task.state = Some(state);
                task.summary = summary;
                task.error = failure_code.map(|code| code.to_string());
                task.blocked_reason = blocked_reason;
                for warning in warnings {
                    task.warn(warning);
                }
                task.artifacts = artifacts;
                task.results = results;
                task.output.activity = None;
                if role == AgentRole::Manager {
                    if !task.artifacts.is_empty()
                        || !task.results.is_empty()
                        || !task.warnings.is_empty()
                    {
                        self.output.push(sequence, Content::Outcome(index));
                    }
                    if matches!(state, AgentRunState::Failed | AgentRunState::Blocked) {
                        self.output.fail(
                            sequence,
                            task.error
                                .clone()
                                .unwrap_or_else(|| "report_delivery_incomplete".into()),
                        );
                    }
                }
            }
            Event::AgentRunInvalidated { run_id } => {
                if let Some(task) = self.tasks.iter_mut().rev().find(|task| task.id == run_id) {
                    task.timing.finish(at);
                    task.output.finish_timing(at, false);
                    task.output.activity = None;
                    task.state = Some(AgentRunState::Stale);
                }
            }
            event => self.output.accept(sequence, at, event)?,
        }
        Ok(())
    }
}
