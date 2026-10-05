use serde::{Deserialize, Serialize};
mod inspection;
pub use inspection::{HarnessResultReferenceDto, HarnessToolInspectionDto};
use yss_harness_contract::{
    AgentEvent, CapabilityId, HarnessEvent, HarnessEventEnvelope, HarnessSessionRecord,
    KnowledgeCitation, StatisticalPlan,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarnessCitationDetailDto {
    pub text: String,
    pub resource: Option<yss_harness_contract::ProjectResourceRef>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveHarnessProviderRequestDto {
    pub config: yss_harness_contract::LanguageModelProviderConfig,
    /// Missing keeps the stored credential; an empty value explicitly clears it.
    pub api_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoverHarnessModelsRequestDto {
    /// Connection draft only; model definitions do not participate in discovery.
    pub config: yss_harness_contract::LanguageModelProviderConfig,
    /// Input-only temporary key. Missing reuses the credential stored for config.id.
    pub api_key: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessSessionDto {
    pub session_id: String,
    pub project_instance_id: String,
    pub project_session_id: String,
    pub title: String,
    pub last_opened_at: u64,
    pub model: Option<yss_harness_contract::LanguageModelSelection>,
}

impl From<HarnessSessionRecord> for HarnessSessionDto {
    fn from(record: HarnessSessionRecord) -> Self {
        Self {
            model: record
                .conversation
                .as_ref()
                .and_then(|value| value.model.clone()),
            title: record
                .conversation
                .as_ref()
                .map(|value| value.title.clone())
                .unwrap_or_default(),
            last_opened_at: record
                .conversation
                .as_ref()
                .map(|value| value.last_opened_at)
                .unwrap_or(record.created_at)
                .get(),
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
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum HarnessEventKindDto {
    ContextCompactionProgress {
        completed_bytes: usize,
        total_bytes: usize,
    },
    GraphExecutionFinished {
        invocation_id: String,
        status: String,
        failure_code: Option<String>,
    },
    AgentRunResumed {
        run_id: String,
        role: yss_harness_contract::AgentRole,
        objective: String,
    },
    TextRetracted {
        characters: usize,
    },
    ContextCompacted,
    RuntimeStatus {
        phase: yss_harness_contract::AgentRuntimePhase,
        attempt: u32,
    },
    DeliveryBlocked {
        reason: String,
    },
    AgentRunInvalidated {
        run_id: String,
    },
    AgentRunStarted {
        run_id: String,
        parent_run_id: Option<String>,
        role: yss_harness_contract::AgentRole,
        objective: String,
    },
    AgentRunOutput {
        run_id: String,
        event: Box<HarnessEventKindDto>,
    },
    AgentRunFinished {
        run_id: String,
        role: yss_harness_contract::AgentRole,
        state: yss_harness_contract::AgentRunState,
        failure_code: Option<yss_harness_contract::AgentDriverFailureCode>,
        summary: Option<String>,
        blocked_reason: Option<String>,
        warnings: Vec<String>,
        evidence_count: usize,
        artifacts: Vec<yss_harness_contract::ResourceChange>,
        results: Vec<HarnessResultReferenceDto>,
    },
    SessionCreated,
    TurnStarted {
        user_message: String,
        model: yss_harness_contract::LanguageModelIdentity,
        resources: Vec<yss_harness_contract::HarnessResourceReference>,
    },
    TextDelta {
        delta: String,
    },
    PlanProposed {
        plan: StatisticalPlan,
    },
    ToolInvocationStarted {
        invocation_id: String,
        capability_id: CapabilityId,
    },
    ToolInvocationCompleted {
        invocation_id: String,
        capability_id: CapabilityId,
    },
    ToolInvocationFailed {
        invocation_id: String,
        capability_id: CapabilityId,
        failure_code: yss_harness_contract::CapabilityFailureCode,
    },
    TurnCompleted {
        final_text: String,
    },
    TurnFailed,
    TurnCancelled,
    KnowledgeCited {
        citation: KnowledgeCitation,
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
            HarnessEvent::AgentRunResumed {
                request,
                role,
                objective,
                ..
            } => Self::AgentRunResumed {
                run_id: request.run_id.to_string(),
                role: *role,
                objective: objective.clone(),
            },
            HarnessEvent::AgentRunInvalidated { run_id } => Self::AgentRunInvalidated {
                run_id: run_id.to_string(),
            },
            HarnessEvent::AgentRunStarted {
                run_id,
                parent_run_id,
                role,
                task,
            } => Self::AgentRunStarted {
                run_id: run_id.to_string(),
                parent_run_id: parent_run_id.as_ref().map(ToString::to_string),
                role: *role,
                objective: task
                    .as_ref()
                    .map(|task| task.objective.clone())
                    .unwrap_or_default(),
            },
            HarnessEvent::AgentRunOutput { run_id, event } => Self::AgentRunOutput {
                run_id: run_id.to_string(),
                event: Box::new(Self::from(event)),
            },
            HarnessEvent::AgentRunFinished { outcome } => Self::AgentRunFinished {
                run_id: outcome.run_id.to_string(),
                role: outcome.role,
                state: outcome.state,
                failure_code: outcome.failure_code,
                summary: outcome.report.as_ref().map(|report| report.summary.clone()),
                blocked_reason: outcome
                    .report
                    .as_ref()
                    .and_then(|report| report.blocked_reason.clone()),
                warnings: outcome
                    .report
                    .as_ref()
                    .map(|report| report.warnings.clone())
                    .unwrap_or_default(),
                evidence_count: outcome.evidence.len(),
                artifacts: outcome.artifacts.clone(),
                results: outcome
                    .results
                    .iter()
                    .cloned()
                    .map(HarnessResultReferenceDto::from)
                    .collect(),
            },
            HarnessEvent::SessionCreated => Self::SessionCreated,
            HarnessEvent::TurnStarted {
                user_message,
                model,
                resources,
            } => Self::TurnStarted {
                user_message: user_message.clone(),
                model: model.clone(),
                resources: resources.clone(),
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
            AgentEvent::KnowledgeCited { citation } => Self::KnowledgeCited {
                citation: citation.clone(),
            },
            AgentEvent::GraphExecutionFinished {
                invocation_id,
                status,
                failure_code,
            } => Self::GraphExecutionFinished {
                invocation_id: invocation_id.to_string(),
                status: status.clone(),
                failure_code: failure_code.clone(),
            },
            AgentEvent::TextRetracted { characters } => Self::TextRetracted {
                characters: *characters,
            },
            // Checkpoint text belongs to model context, not the user's transcript.
            AgentEvent::ContextCompacted { .. } => Self::ContextCompacted,
            AgentEvent::ContextCompactionProgress {
                completed_bytes,
                total_bytes,
                ..
            } => Self::ContextCompactionProgress {
                completed_bytes: *completed_bytes,
                total_bytes: *total_bytes,
            },
            AgentEvent::RuntimeStatus { phase, attempt } => Self::RuntimeStatus {
                phase: *phase,
                attempt: *attempt,
            },
            AgentEvent::DeliveryBlocked { reason } => Self::DeliveryBlocked {
                reason: reason.clone(),
            },
            AgentEvent::TextDelta { delta } => Self::TextDelta {
                delta: delta.clone(),
            },
            AgentEvent::PlanProposed { plan } => Self::PlanProposed { plan: plan.clone() },
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
            AgentEvent::ToolInvocationFailed {
                invocation_id,
                capability_id,
                failure_code,
            } => Self::ToolInvocationFailed {
                invocation_id: invocation_id.to_string(),
                capability_id: *capability_id,
                failure_code: *failure_code,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_events_match_the_frontend_wire_fixture() {
        let knowledge: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../src/tests/fixtures/node-system-contracts/harness-knowledge.json"
        ))
        .unwrap();
        let sources: Vec<yss_harness_contract::ProjectKnowledgeSourceSummary> =
            serde_json::from_value(knowledge["sources"].clone()).unwrap();
        assert_eq!(serde_json::to_value(sources).unwrap(), knowledge["sources"]);
        for key in ["citation", "builtin"] {
            let detail: HarnessCitationDetailDto =
                serde_json::from_value(knowledge[key].clone()).unwrap();
            assert_eq!(serde_json::to_value(detail).unwrap(), knowledge[key]);
        }
        let events = [
            HarnessEventKindDto::SessionCreated,
            HarnessEventKindDto::TurnStarted {
                resources: vec![],
                model: yss_harness_contract::LanguageModelIdentity {
                    selection: yss_harness_contract::LanguageModelSelection {
                        provider_id: "test-provider".into(),
                        model_id: "test-model".into(),
                    },
                    provider_name: "Test provider".into(),
                    model_name: "Test model".into(),
                },
                user_message: "Inspect the dataset".into(),
            },
            HarnessEventKindDto::ToolInvocationStarted {
                invocation_id: "tool-1".into(),
                capability_id: CapabilityId::InspectDatasetSchema,
            },
            HarnessEventKindDto::TurnCompleted {
                final_text: "Schema inspected.".into(),
            },
            HarnessEventKindDto::WorkflowStepFailed {
                run_id: "workflow-1".into(),
                step_id: "schema".into(),
                retriable: true,
            },
            HarnessEventKindDto::ToolInvocationFailed {
                invocation_id: "tool-2".into(),
                capability_id: CapabilityId::InspectDatasetProfile,
                failure_code: yss_harness_contract::CapabilityFailureCode::DeadlineElapsed,
            },
            HarnessEventKindDto::AgentRunResumed {
                run_id: "worker-1".into(),
                role: yss_harness_contract::AgentRole::Report,
                objective: "Save report".into(),
            },
            HarnessEventKindDto::RuntimeStatus {
                phase: yss_harness_contract::AgentRuntimePhase::Reconnecting,
                attempt: 1,
            },
            HarnessEventKindDto::ContextCompacted,
            HarnessEventKindDto::TextRetracted { characters: 7 },
            HarnessEventKindDto::GraphExecutionFinished {
                invocation_id: "tool-3".into(),
                status: "failed".into(),
                failure_code: Some("resource_version_changed".into()),
            },
            HarnessEventKindDto::DeliveryBlocked {
                reason: "report_document_not_saved".into(),
            },
            HarnessEventKindDto::AgentRunFinished {
                run_id: "worker-1".into(),
                role: yss_harness_contract::AgentRole::Report,
                state: yss_harness_contract::AgentRunState::Failed,
                failure_code: Some(
                    yss_harness_contract::AgentDriverFailureCode::ProviderPaymentRequired,
                ),
                summary: None,
                blocked_reason: None,
                warnings: vec![],
                evidence_count: 3,
                artifacts: vec![],
                results: vec![],
            },
        ]
        .into_iter()
        .enumerate()
        .map(|(index, event)| HarnessEventDto {
            sequence: index as u64 + 1,
            session_id: "session-1".into(),
            turn_id: ((1..=3).contains(&index) || index >= 5).then(|| "turn-1".into()),
            occurred_at: 1000,
            event,
        })
        .collect::<Vec<_>>();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../src/tests/fixtures/node-system-contracts/harness-events.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(events).unwrap(), fixture);
        let progress = AgentEvent::ContextCompactionProgress {
            completed_bytes: 50,
            total_bytes: 100,
            checkpoint: Some(yss_harness_contract::ContextCompactionCheckpoint {
                processed_bytes: 50,
                prefix_hash: "private-prefix".into(),
                summary: "private-model-context".into(),
            }),
        };
        assert_eq!(
            serde_json::to_value(HarnessEventKindDto::from(&progress)).unwrap(),
            serde_json::json!({
                "type":"context_compaction_progress", "payload":{"completedBytes":50,"totalBytes":100}
            })
        );
    }
}
