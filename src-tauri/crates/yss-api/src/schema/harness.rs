use serde::{Deserialize, Serialize};
use yss_automation_contract::{
    AgentEvent, CapabilityId, HarnessEvent, HarnessEventEnvelope, HarnessSessionRecord,
    KnowledgeCitation, MemoryKind, MemoryRecord, MemoryScope, MemoryStatus, StatisticalPlan,
    StructuredMemoryValue, WorkflowRunRecord, WorkflowRunState,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessRuntimeStatusDto {
    pub provider_configured: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigureHarnessProviderRequestDto {
    pub model: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessSessionDto {
    pub session_id: String,
    pub project_instance_id: String,
    pub project_session_id: String,
}

impl From<HarnessSessionRecord> for HarnessSessionDto {
    fn from(record: HarnessSessionRecord) -> Self {
        Self {
            session_id: record.id.to_string(),
            project_instance_id: record.project.project_instance_id().as_str().to_owned(),
            project_session_id: record.project.project_session_id().as_str().to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessTurnResultDto {
    pub final_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessSubscriptionDto {
    pub subscription_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunDto {
    pub run_id: String,
    pub state: WorkflowRunState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessMemoryRecordDto {
    pub record_id: String,
    pub scope: MemoryScope,
    pub kind: MemoryKind,
    pub status: MemoryStatus,
    pub value: StructuredMemoryValue,
    pub created_at: u64,
    pub updated_at: u64,
}

impl From<MemoryRecord> for HarnessMemoryRecordDto {
    fn from(record: MemoryRecord) -> Self {
        Self {
            record_id: record.id.to_string(),
            scope: record.scope,
            kind: record.kind,
            status: record.status,
            value: record.value,
            created_at: record.created_at.get(),
            updated_at: record.updated_at.get(),
        }
    }
}

impl From<WorkflowRunRecord> for WorkflowRunDto {
    fn from(record: WorkflowRunRecord) -> Self {
        Self {
            run_id: record.id.to_string(),
            state: record.state,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessEventDto {
    pub sequence: u64,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub occurred_at: u64,
    #[serde(flatten)]
    pub event: HarnessEventKindDto,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum HarnessEventKindDto {
    SessionCreated,
    SessionClosed,
    TurnStarted {
        user_message: String,
    },
    TextDelta {
        delta: String,
    },
    PlanProposed {
        plan: StatisticalPlan,
    },
    ToolInvocationRequested {
        capability_id: CapabilityId,
    },
    ToolInvocationStarted {
        invocation_id: String,
        capability_id: CapabilityId,
    },
    ToolInvocationCompleted {
        invocation_id: String,
        capability_id: CapabilityId,
    },
    TurnCompleted {
        final_text: String,
    },
    TurnFailed,
    TurnCancelled,
    KnowledgeCited {
        citation: KnowledgeCitation,
    },
    MemoryRecorded {
        record: HarnessMemoryRecordDto,
    },
    MemoryDeleted {
        record_id: String,
    },
    WorkflowPlanned {
        run_id: String,
    },
    WorkflowStarted {
        run_id: String,
    },
    WorkflowStepStarted {
        run_id: String,
        step_id: String,
    },
    WorkflowStepCompleted {
        run_id: String,
        step_id: String,
    },
    WorkflowStepFailed {
        run_id: String,
        step_id: String,
        retriable: bool,
    },
    WorkflowCompleted {
        run_id: String,
    },
    WorkflowPaused {
        run_id: String,
    },
    WorkflowResumed {
        run_id: String,
    },
    WorkflowCancelled {
        run_id: String,
    },
}

impl From<&HarnessEventEnvelope> for HarnessEventDto {
    fn from(envelope: &HarnessEventEnvelope) -> Self {
        Self {
            sequence: envelope.sequence,
            session_id: envelope.session_id.to_string(),
            turn_id: envelope.turn_id.as_ref().map(ToString::to_string),
            occurred_at: envelope.occurred_at.get(),
            event: HarnessEventKindDto::from(&envelope.event),
        }
    }
}

impl From<&HarnessEvent> for HarnessEventKindDto {
    fn from(event: &HarnessEvent) -> Self {
        match event {
            HarnessEvent::SessionCreated => Self::SessionCreated,
            HarnessEvent::SessionClosed => Self::SessionClosed,
            HarnessEvent::TurnStarted { user_message } => Self::TurnStarted {
                user_message: user_message.clone(),
            },
            HarnessEvent::Agent(event) => Self::from(event),
            HarnessEvent::TurnCompleted { final_text } => Self::TurnCompleted {
                final_text: final_text.clone(),
            },
            HarnessEvent::TurnFailed => Self::TurnFailed,
            HarnessEvent::TurnCancelled => Self::TurnCancelled,
            HarnessEvent::KnowledgeCited { citation } => Self::KnowledgeCited {
                citation: citation.clone(),
            },
            HarnessEvent::MemoryRecorded { record } => Self::MemoryRecorded {
                record: record.clone().into(),
            },
            HarnessEvent::MemoryDeleted { record_id } => Self::MemoryDeleted {
                record_id: record_id.to_string(),
            },
            HarnessEvent::WorkflowPlanned { run_id } => Self::WorkflowPlanned {
                run_id: run_id.to_string(),
            },
            HarnessEvent::WorkflowStarted { run_id } => Self::WorkflowStarted {
                run_id: run_id.to_string(),
            },
            HarnessEvent::WorkflowStepStarted { run_id, step_id } => Self::WorkflowStepStarted {
                run_id: run_id.to_string(),
                step_id: step_id.to_string(),
            },
            HarnessEvent::WorkflowStepCompleted { run_id, step_id } => {
                Self::WorkflowStepCompleted {
                    run_id: run_id.to_string(),
                    step_id: step_id.to_string(),
                }
            }
            HarnessEvent::WorkflowStepFailed {
                run_id,
                step_id,
                retriable,
            } => Self::WorkflowStepFailed {
                run_id: run_id.to_string(),
                step_id: step_id.to_string(),
                retriable: *retriable,
            },
            HarnessEvent::WorkflowCompleted { run_id } => Self::WorkflowCompleted {
                run_id: run_id.to_string(),
            },
            HarnessEvent::WorkflowPaused { run_id } => Self::WorkflowPaused {
                run_id: run_id.to_string(),
            },
            HarnessEvent::WorkflowResumed { run_id } => Self::WorkflowResumed {
                run_id: run_id.to_string(),
            },
            HarnessEvent::WorkflowCancelled { run_id } => Self::WorkflowCancelled {
                run_id: run_id.to_string(),
            },
        }
    }
}

impl From<&AgentEvent> for HarnessEventKindDto {
    fn from(event: &AgentEvent) -> Self {
        match event {
            AgentEvent::TextDelta { delta } => Self::TextDelta {
                delta: delta.clone(),
            },
            AgentEvent::PlanProposed { plan } => Self::PlanProposed { plan: plan.clone() },
            AgentEvent::ToolInvocationRequested { capability_id } => {
                Self::ToolInvocationRequested {
                    capability_id: *capability_id,
                }
            }
            AgentEvent::ToolInvocationStarted {
                invocation_id,
                capability_id,
            } => Self::ToolInvocationStarted {
                invocation_id: invocation_id.to_string(),
                capability_id: *capability_id,
            },
            AgentEvent::ToolInvocationCompleted {
                invocation_id,
                capability_id,
            } => Self::ToolInvocationCompleted {
                invocation_id: invocation_id.to_string(),
                capability_id: *capability_id,
            },
        }
    }
}
