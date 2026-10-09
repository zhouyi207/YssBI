//! Tool outcomes are reduced from events; a completed invocation can contain a failed graph run.
use super::Timing;
use yss_harness_contract::AssistantToolIdentity;

#[derive(Clone, Copy, PartialEq)]
pub(in crate::assistant) enum ToolState {
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
    Interrupted,
    Unknown,
    GraphSucceeded,
    GraphFailed,
}

#[derive(Clone, PartialEq)]
pub(in crate::assistant) struct Tool {
    pub id: String,
    pub kind: AssistantToolIdentity,
    pub state: ToolState,
    pub failure: Option<String>,
    pub timing: Option<Timing>,
}

impl Tool {
    pub fn recovered(id: String, kind: AssistantToolIdentity) -> Self {
        Self {
            id,
            kind,
            state: ToolState::Running,
            failure: None,
            timing: None,
        }
    }

    pub fn started(id: String, kind: AssistantToolIdentity, at: u64) -> Self {
        Self {
            timing: Some(Timing::started(at)),
            ..Self::recovered(id, kind)
        }
    }

    pub fn running(&self) -> bool {
        self.state == ToolState::Running
    }

    pub fn completed(&mut self, at: u64) {
        // Keep an already recorded graph outcome when completing its invocation.
        if self.running() {
            self.state = ToolState::Completed;
        }
        self.finish_timing(at);
    }

    pub fn failed(&mut self, at: u64, code: String) {
        self.state = match code.as_str() {
            "cancelled" => ToolState::Cancelled,
            "deadline_elapsed" => ToolState::TimedOut,
            "outcome_unknown" => ToolState::Unknown,
            _ => ToolState::Failed,
        };
        self.failure = Some(code);
        self.finish_timing(at);
    }

    pub fn executed(&mut self, at: u64, status: &str, failure: Option<String>) {
        self.state = match status {
            "succeeded" => ToolState::GraphSucceeded,
            "failed" => ToolState::GraphFailed,
            "cancelled" => ToolState::Cancelled,
            _ => ToolState::Unknown,
        };
        self.failure = failure.or(self.failure.take());
        self.finish_timing(at);
    }

    pub fn settle(&mut self, at: u64, cancelled: bool) {
        if self.running() {
            self.state = if cancelled {
                ToolState::Cancelled
            } else {
                ToolState::Interrupted
            };
            self.finish_timing(at);
        }
    }

    fn finish_timing(&mut self, at: u64) {
        if let Some(timing) = &mut self.timing {
            timing.finish(at);
        }
    }
}
