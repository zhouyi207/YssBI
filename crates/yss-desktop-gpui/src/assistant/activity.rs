//! Current activity is an event projection; presentation reads the current locale.
use yss_harness_contract::{AgentRuntimePhase, AssistantEventKind as Event, AssistantToolIdentity};

#[derive(Clone)]
pub(super) enum Activity {
    Writing,
    Thinking,
    Runtime {
        phase: AgentRuntimePhase,
        attempt: u32,
    },
    Compaction {
        completed: usize,
        total: usize,
    },
    Tool(AssistantToolIdentity),
}

impl Activity {
    pub fn accept(current: &mut Option<Self>, event: &Event) {
        *current = match event {
            Event::TextDelta { delta } if !delta.is_empty() => Some(Self::Writing),
            Event::ReasoningDelta { delta } if !delta.is_empty() => Some(Self::Thinking),
            Event::TextRetracted { characters } if *characters > 0 => Some(Self::Writing),
            Event::RuntimeStatus { phase, attempt } => Some(Self::Runtime {
                phase: *phase,
                attempt: *attempt,
            }),
            Event::ContextCompactionProgress {
                completed_bytes,
                total_bytes,
            } => Some(Self::Compaction {
                completed: *completed_bytes,
                total: *total_bytes,
            }),
            Event::ToolInvocationStarted { capability_id, .. } => Some(Self::Tool(*capability_id)),
            Event::ContextCompacted { .. }
            | Event::ToolInvocationCompleted { .. }
            | Event::ToolInvocationFailed { .. }
            | Event::GraphExecutionFinished { .. } => None,
            _ => return,
        };
    }

    pub fn label(&self) -> String {
        match self {
            Self::Writing => crate::text::t("panel.assistantToolNames.writing").into(),
            Self::Thinking => crate::text::t("panel.assistantThinking").into(),
            Self::Compaction { .. } => crate::text::t("panel.assistantToolNames.compacting").into(),
            Self::Runtime { phase, .. } => crate::text::t(match phase {
                AgentRuntimePhase::CheckingDelivery => "panel.assistantToolNames.checking_delivery",
                AgentRuntimePhase::Reconnecting => "panel.assistantToolNames.reconnecting",
                AgentRuntimePhase::Compacting => "panel.assistantToolNames.compacting",
            })
            .into(),
            Self::Tool(kind) => tool_name(*kind),
        }
    }

    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Runtime {
                phase: AgentRuntimePhase::Reconnecting,
                attempt,
            } if *attempt > 0 => Some(crate::text::format(
                "panel.assistantRecoveryAttempt",
                &[("count", attempt.to_string())],
            )),
            Self::Compaction { completed, total } if *total > 0 => Some(format!(
                "{}%",
                ((*completed as u128) * 100 / (*total as u128)).min(100)
            )),
            _ => None,
        }
    }
}

pub(super) fn tool_name(kind: AssistantToolIdentity) -> String {
    let code = match kind {
        AssistantToolIdentity::Capability(kind) => kind.as_str(),
        AssistantToolIdentity::Control(kind) => kind.as_str(),
    };
    let key = format!("panel.assistantToolNames.{code}");
    let label = crate::text::translate(&key);
    if label == key { code.to_owned() } else { label }
}
