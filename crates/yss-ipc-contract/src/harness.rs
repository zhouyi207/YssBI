use serde::Serialize;
mod inspection;
pub use inspection::{HarnessResultReferenceDto, HarnessToolInspectionDto};
use yss_harness_contract::{
    AgentEvent, CapabilityId, HarnessEvent, HarnessEventEnvelope, KnowledgeCitation,
    StatisticalPlan,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum HarnessToolIdentityDto {
    Capability(CapabilityId),
    Control(yss_harness_contract::AgentControlTool),
}

impl From<CapabilityId> for HarnessToolIdentityDto {
    fn from(value: CapabilityId) -> Self {
        Self::Capability(value)
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
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum HarnessEventKindDto {
    TurnConfigured {
        options: yss_harness_contract::HarnessTurnOptions,
    },
    ReasoningDelta {
        delta: String,
    },
    UsageReported {
        usage: yss_harness_contract::ModelTokenUsage,
        context_window: Option<u32>,
        purpose: yss_harness_contract::ModelCallPurpose,
    },
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
        capability_id: HarnessToolIdentityDto,
    },
    ToolInvocationCompleted {
        invocation_id: String,
        capability_id: HarnessToolIdentityDto,
    },
    ToolInvocationFailed {
        invocation_id: String,
        capability_id: HarnessToolIdentityDto,
        failure_code: String,
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
            HarnessEvent::TurnConfigured { options } => Self::TurnConfigured { options: *options },
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
            AgentEvent::ReasoningDelta { delta } => Self::ReasoningDelta {
                delta: delta.clone(),
            },
            AgentEvent::UsageReported {
                usage,
                context_window,
                purpose,
            } => Self::UsageReported {
                usage: usage.clone(),
                context_window: *context_window,
                purpose: *purpose,
            },
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
            AgentEvent::ControlToolStarted {
                invocation_id,
                tool,
            } => Self::ToolInvocationStarted {
                invocation_id: invocation_id.to_string(),
                capability_id: HarnessToolIdentityDto::Control(*tool),
            },
            AgentEvent::ControlToolFinished {
                invocation_id,
                tool,
                failure_code,
                failure_details,
            } => {
                let invocation_id = invocation_id.to_string();
                let capability_id = HarnessToolIdentityDto::Control(*tool);
                match failure_code {
                    Some(failure_code) => Self::ToolInvocationFailed {
                        invocation_id,
                        capability_id,
                        failure_code: yss_harness_contract::model::failure_code(
                            &yss_harness_contract::CapabilityFailure {
                                code: *failure_code,
                                details: failure_details.clone().unwrap_or_default(),
                            },
                        ),
                    },
                    None => Self::ToolInvocationCompleted {
                        invocation_id,
                        capability_id,
                    },
                }
            }
            AgentEvent::ToolInvocationStarted {
                invocation_id,
                capability_id,
            } => Self::ToolInvocationStarted {
                invocation_id: invocation_id.to_string(),
                capability_id: (*capability_id).into(),
            },
            AgentEvent::ToolInvocationCompleted {
                invocation_id,
                capability_id,
            } => Self::ToolInvocationCompleted {
                invocation_id: invocation_id.to_string(),
                capability_id: (*capability_id).into(),
            },
            AgentEvent::ToolInvocationFailed {
                invocation_id,
                capability_id,
                failure_code,
                failure_details,
            } => Self::ToolInvocationFailed {
                invocation_id: invocation_id.to_string(),
                capability_id: (*capability_id).into(),
                failure_code: yss_harness_contract::model::failure_code(
                    &yss_harness_contract::CapabilityFailure {
                        code: *failure_code,
                        details: failure_details.clone().unwrap_or_default(),
                    },
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_failures_share_public_codes_in_events_and_inspection() {
        use yss_harness_contract::{
            AgentControlTool, CapabilityFailureCode, HarnessSessionId, ToolInvocationId, UnixMillis,
        };
        let invocation_id = ToolInvocationId::try_new("delegation").unwrap();
        for (code, reason, expected) in [
            (
                CapabilityFailureCode::RevisionConflict,
                "resource_requires_current_read",
                "resource_read_required",
            ),
            (
                CapabilityFailureCode::InvalidRequest,
                "resource_read_required",
                "resource_read_required",
            ),
            (
                CapabilityFailureCode::RevisionConflict,
                "dataset_changed_since_read",
                "resource_changed",
            ),
        ] {
            let details: std::collections::BTreeMap<String, String> = [
                ("reason".into(), reason.into()),
                ("resourceId".into(), "database-1".into()),
                ("expectedRevision".into(), "private-version".into()),
            ]
            .into();
            let business = AgentEvent::ToolInvocationFailed {
                invocation_id: invocation_id.clone(),
                capability_id: CapabilityId::SaveResource,
                failure_code: code,
                failure_details: Some(details.clone()),
            };
            let replay: AgentEvent =
                serde_json::from_value(serde_json::to_value(&business).unwrap()).unwrap();
            assert_eq!(business, replay);
            let business_wire = serde_json::to_value(HarnessEventKindDto::from(&replay)).unwrap();
            assert_eq!(business_wire["payload"]["failureCode"], expected);
            let finished = AgentEvent::ControlToolFinished {
                invocation_id: invocation_id.clone(),
                tool: AgentControlTool::DelegateTask,
                failure_code: Some(code),
                failure_details: Some(details),
            };
            let wire = serde_json::to_value(HarnessEventKindDto::from(&finished)).unwrap();
            assert_eq!(wire["payload"]["failureCode"], expected);
            let events = [
                AgentEvent::ControlToolStarted {
                    invocation_id: invocation_id.clone(),
                    tool: AgentControlTool::DelegateTask,
                },
                finished,
            ]
            .into_iter()
            .enumerate()
            .map(|(index, event)| HarnessEventEnvelope {
                sequence: index as u64 + 1,
                session_id: HarnessSessionId::try_new("session").unwrap(),
                turn_id: None,
                occurred_at: UnixMillis::from_existing(1000 + index as u64),
                event: HarnessEvent::Agent(event),
            })
            .collect::<Vec<_>>();
            let replay: Vec<HarnessEventEnvelope> =
                serde_json::from_value(serde_json::to_value(events).unwrap()).unwrap();
            let inspection =
                HarnessToolInspectionDto::from_control_events(&replay, &invocation_id).unwrap();
            let failure = inspection.failure.unwrap();
            assert_eq!(failure["code"], expected);
            assert_eq!(failure["details"]["resourceId"], "database-1");
            for value in [wire, business_wire, failure] {
                let encoded = value.to_string();
                assert!(
                    !encoded.contains("revision")
                        && !encoded.contains("Revision")
                        && !encoded.contains("private-version")
                );
            }
        }
        let event = AgentEvent::ToolInvocationFailed {
            invocation_id,
            capability_id: CapabilityId::SaveResource,
            failure_code: CapabilityFailureCode::RevisionConflict,
            failure_details: None,
        };
        let wire = serde_json::to_value(HarnessEventKindDto::from(&event)).unwrap();
        assert_eq!(wire["payload"]["failureCode"], "resource_changed");
    }

    #[test]
    fn control_events_replay_exact_tool_identity_failure_and_inspection_times() {
        use yss_harness_contract::{
            AgentControlTool, CapabilityFailureCode, HarnessSessionId, ToolInvocationId, UnixMillis,
        };
        let invocation_id = ToolInvocationId::try_new("control-call").unwrap();
        let events = [
            AgentEvent::ControlToolStarted {
                invocation_id: invocation_id.clone(),
                tool: AgentControlTool::DelegateTask,
            },
            AgentEvent::ControlToolFinished {
                invocation_id: invocation_id.clone(),
                tool: AgentControlTool::DelegateTask,
                failure_code: Some(CapabilityFailureCode::InvalidRequest),
                failure_details: Some(
                    [
                        ("category".into(), "missing_field".into()),
                        ("path".into(), "$.constraints".into()),
                        ("expected".into(), "type=string".into()),
                        ("expectedRevision".into(), "private-version".into()),
                    ]
                    .into(),
                ),
            },
        ]
        .into_iter()
        .enumerate()
        .map(|(index, event)| HarnessEventEnvelope {
            sequence: index as u64 + 1,
            session_id: HarnessSessionId::try_new("session").unwrap(),
            turn_id: None,
            occurred_at: UnixMillis::from_existing(1000 + index as u64 * 37),
            event: HarnessEvent::Agent(event),
        })
        .collect::<Vec<_>>();
        let replay: Vec<HarnessEventEnvelope> =
            serde_json::from_value(serde_json::to_value(&events).unwrap()).unwrap();
        let inspection =
            HarnessToolInspectionDto::from_control_events(&replay, &invocation_id).unwrap();
        assert_eq!(inspection.started_at, 1000);
        assert_eq!(inspection.finished_at, Some(1037));
        assert!(inspection.parameters.is_empty() && inspection.artifacts.is_empty());
        assert_eq!(
            inspection.failure,
            Some(serde_json::json!({
                "code": "invalid_request", "details": {"category": "missing_field", "path": "$.constraints", "expected": "type=string"}
            }))
        );
        let HarnessEvent::Agent(event) = &replay[1].event else {
            panic!("control event")
        };
        let wire = serde_json::to_value(HarnessEventKindDto::from(event)).unwrap();
        assert_eq!(wire["type"], "tool_invocation_failed");
        assert_eq!(wire["payload"]["capabilityId"], "delegate_task");
        assert_eq!(wire["payload"]["failureCode"], "invalid_request");
        assert!(
            HarnessToolInspectionDto::from_control_events(
                &replay,
                &ToolInvocationId::try_new("absent").unwrap()
            )
            .is_none()
        );
    }

    #[test]
    fn harness_event_serialization_preserves_public_facts_and_hides_checkpoints() {
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
                capability_id: CapabilityId::InspectDatasetSchema.into(),
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
                capability_id: CapabilityId::InspectDatasetProfile.into(),
                failure_code: yss_harness_contract::CapabilityFailureCode::DeadlineElapsed
                    .to_string(),
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
                reason: "report_resource_not_saved".into(),
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
            HarnessEventKindDto::TurnConfigured {
                options: yss_harness_contract::HarnessTurnOptions {
                    mode: yss_harness_contract::HarnessMode::Ask,
                    reasoning_effort: Some(yss_harness_contract::ReasoningEffort::High),
                },
            },
            HarnessEventKindDto::ReasoningDelta {
                delta: "Checking evidence".into(),
            },
            HarnessEventKindDto::UsageReported {
                usage: yss_harness_contract::ModelTokenUsage {
                    input_tokens: Some(100),
                    output_tokens: Some(20),
                    cached_input_tokens: Some(60),
                    reasoning_tokens: Some(10),
                    cache_creation_input_tokens: None,
                },
                context_window: Some(1000),
                purpose: yss_harness_contract::ModelCallPurpose::Response,
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
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("harness/fixtures/events.json")).unwrap();
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
