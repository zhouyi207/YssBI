use crate::skills::{STATISTICAL_REPORT_WRITING_ID, STATISTICAL_REPORT_WRITING_VERSION};
use crate::{ApprovalError, HarnessError, HarnessHost, HarnessPorts, SkillRegistry, ToolRegistry};
use yss_harness_contract::*;

use std::sync::Arc;

use super::*;
use crate::test_support::{
    FixedClock, InMemoryHarnessStore, MockAgentDriver, RejectingCapabilityGateway, SequentialIds,
    StaticCapabilityGateway,
};
use yss_harness_contract::{
    AgentTurnRequest, ApplyGraphEditRequest, AutomationCapabilityRequest,
    AutomationCapabilityResult, CapabilityFuture, CapabilityGatewayPort,
    CapabilityInvocationContext, DatasetProfileInspection, DatasetSchemaInspection,
    GraphEditOperation, GraphEditPosition, GraphEditReceipt, WorkflowRunState,
};
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

#[test]
fn cancellation_releases_active_turns_before_waking_reentrant_waiters() {
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Wake, Waker};

    struct ReentrantWake {
        host: std::sync::Weak<HarnessHost>,
        session_id: HarnessSessionId,
        calls: AtomicUsize,
    }
    impl Wake for ReentrantWake {
        fn wake(self: Arc<Self>) {
            let host = self.host.upgrade().unwrap();
            assert!(
                host.active_turns.try_lock().is_ok(),
                "cancellation must release active_turns before waking waiters"
            );
            assert!(!host.cancel_turn(&self.session_id));
            self.calls.fetch_add(1, Ordering::Relaxed);
        }
    }

    let store = Arc::new(InMemoryHarnessStore::default());
    let host = Arc::new(
        HarnessHost::new(HarnessPorts {
            resources: Arc::new(crate::test_support::FixtureResourceResolver),
            models: crate::test_support::fixed_model(Arc::new(MockAgentDriver::new("unused"))),
            capability_gateway: Arc::new(RejectingCapabilityGateway),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: store.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
            approvals: store,
            clock: Arc::new(FixedClock::new(1_000)),
            ids: Arc::new(SequentialIds::default()),
        })
        .unwrap(),
    );
    let session_id = HarnessSessionId::try_new("session-1").unwrap();
    for reason in [
        CancellationReason::User,
        CancellationReason::ProjectReplaced,
    ] {
        let (token, admission) = host.admit_turn(&session_id).unwrap();
        let wake = Arc::new(ReentrantWake {
            host: Arc::downgrade(&host),
            session_id: session_id.clone(),
            calls: AtomicUsize::new(0),
        });
        let waker = Waker::from(wake.clone());
        let mut cancelled = Box::pin(token.cancelled());
        assert!(
            cancelled
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );

        let accepted = match reason {
            CancellationReason::User => host.cancel_turn(&session_id),
            _ => host.cancel_active_turn(&session_id, reason),
        };
        assert!(accepted);
        assert_eq!(token.reason(), Some(reason));
        assert_eq!(wake.calls.load(Ordering::Relaxed), 1);
        assert!(!host.cancel_turn(&session_id));
        drop(admission);
        assert!(!host.cancel_turn(&session_id));
    }
}

#[tokio::test]
async fn session_turn_persists_one_gap_free_ordered_event_stream() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = HarnessHost::new(HarnessPorts {
        resources: Arc::new(crate::test_support::FixtureResourceResolver),
        models: crate::test_support::fixed_model(Arc::new(MockAgentDriver::new(
            "Evidence is ready.",
        ))),
        capability_gateway: Arc::new(RejectingCapabilityGateway),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        workflows: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
        approvals: store.clone(),
        clock: Arc::new(FixedClock::new(1_000)),
        ids: Arc::new(SequentialIds::default()),
    })
    .unwrap();
    let session = host
        .create_session(
            PrincipalId::try_new("user-1").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-1"),
            ),
        )
        .await
        .unwrap();

    let references = vec![yss_harness_contract::HarnessResourceReference {
        resource: yss_harness_contract::ProjectResourceRef {
            kind: yss_harness_contract::ProjectResourceKind::Database,
            id: "database-a".into(),
        },
        name: "database-a".into(),
    }];
    let result = host
        .submit_turn(
            &session.id,
            &session.project,
            "Review the dataset.".to_owned(),
            references
                .iter()
                .map(|entry| entry.resource.clone())
                .collect(),
            None,
        )
        .await
        .unwrap();
    let events = host.events_after(&session.id, 0).await.unwrap();

    assert_eq!(result.final_text, "Evidence is ready.");
    assert!(events.iter().any(|event| matches!(&event.event,
        HarnessEvent::AgentRunFinished { outcome }
            if outcome.role == yss_harness_contract::AgentRole::Manager
                && outcome.state == yss_harness_contract::AgentRunState::Completed
                && outcome.report.is_none()
    )));
    assert_eq!(
        events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        (1..=events.len() as u64).collect::<Vec<_>>()
    );
    assert!(matches!(events[0].event, HarnessEvent::SessionCreated));
    assert!(matches!(
        events.last().unwrap().event,
        HarnessEvent::TurnCompleted { .. }
    ));
    assert_eq!(store.published_events(), events);
    assert!(events.iter().any(|event| matches!(&event.event, HarnessEvent::TurnStarted { resources, .. } if resources == &references)));
}

#[tokio::test]
async fn conversation_rename_preserves_history_and_rejects_active_turns() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = HarnessHost::new(HarnessPorts {
        resources: Arc::new(crate::test_support::FixtureResourceResolver),
        models: crate::test_support::fixed_model(Arc::new(MockAgentDriver::new("unused"))),
        capability_gateway: Arc::new(RejectingCapabilityGateway),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        workflows: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
        approvals: store,
        clock: Arc::new(FixedClock::new(1000)),
        ids: Arc::new(SequentialIds::default()),
    })
    .unwrap();
    let principal = PrincipalId::try_new("user").unwrap();
    let session = host
        .session_access()
        .await
        .create_conversation(
            principal.clone(),
            "project-key".into(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-1"),
            ),
        )
        .await
        .unwrap();
    let before = host.events_after(&session.id, 0).await.unwrap();
    let (_cancel, admission) = host.admit_turn(&session.id).unwrap();
    assert!(matches!(
        host.session_access()
            .await
            .rename_conversation(&session.id, &principal, "project-key", "Renamed".into())
            .await,
        Err(HarnessError::ConcurrentTurn)
    ));
    drop(admission);
    let renamed = host
        .session_access()
        .await
        .rename_conversation(&session.id, &principal, "project-key", "  Renamed  ".into())
        .await
        .unwrap();
    assert_eq!(renamed.conversation.unwrap().title, "Renamed");
    assert_eq!(host.events_after(&session.id, 0).await.unwrap(), before);
    assert!(matches!(
        host.session_access()
            .await
            .rename_conversation(&session.id, &principal, "other-project", "Wrong".into())
            .await,
        Err(HarnessError::SessionNotFound)
    ));
}

#[tokio::test]
async fn complete_conversation_survives_twelve_turns_with_tools_failures_and_cancellation() {
    use yss_harness_contract::{
        AgentFuture, InspectProjectRequest, InspectResultRequest, ResultCategoryInspection,
        ResultInspection, ResultValueInspection,
    };
    struct RecordingDriver(Mutex<Vec<AgentTurnRequest>>);
    impl AgentDriverPort for RecordingDriver {
        fn run_turn<'a>(
            &'a self,
            request: AgentTurnRequest,
            capabilities: Arc<dyn ModelCapabilityExecutor>,
            output: Arc<dyn AgentEventOutput>,
            cancellation: CancellationToken,
        ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
            Box::pin(async move {
                let index = {
                    let mut requests = self.0.lock().unwrap();
                    let index = requests.len();
                    requests.push(request);
                    index
                };
                let before = format!("checking-{index}:{}:before-tail", "b".repeat(2048));
                output
                    .emit(AgentEvent::TextDelta {
                        delta: before.clone(),
                    })
                    .await
                    .unwrap();
                capabilities
                    .execute(ModelCapabilityRequest {
                        request: (AutomationCapabilityRequest::InspectResult(
                            InspectResultRequest {
                                execution_session_id: "00000000-0000-0000-0000-000000000001".into(),
                                result_id: 7,
                                part: None,
                                offset: 0,
                                limit: 20,
                            },
                        ))
                        .into(),
                    })
                    .await
                    .unwrap();
                assert!(
                    capabilities
                        .execute(ModelCapabilityRequest {
                            request: (AutomationCapabilityRequest::InspectProject(
                                InspectProjectRequest {}
                            ))
                            .into(),
                        })
                        .await
                        .is_err()
                );
                let after = format!("answer-{index}:{}:answer-tail-{index}", "a".repeat(2048));
                output
                    .emit(AgentEvent::TextDelta {
                        delta: after.clone(),
                    })
                    .await
                    .unwrap();
                if index == 1 {
                    cancellation.cancel(CancellationReason::User);
                    return Err(AgentDriverFailure::new(AgentDriverFailureCode::Cancelled));
                }
                if index == 2 {
                    return Err(AgentDriverFailure::new(
                        AgentDriverFailureCode::InvalidProviderResponse,
                    ));
                }
                Ok(AgentTurnResult {
                    final_text: before + &after,
                })
            })
        }
    }
    let store = Arc::new(InMemoryHarnessStore::default());
    let driver = Arc::new(RecordingDriver(Mutex::new(Vec::new())));
    let evidence = "full-tool-result:".to_owned() + &"v".repeat(8192) + ":result-tail";
    let result = AutomationCapabilityResult::ResultInspection(ResultInspection {
        result_id: 7,
        category: ResultCategoryInspection::Value,
        value: ResultValueInspection::Json(evidence.clone().into()),
    });
    let host = HarnessHost::new(HarnessPorts {
        resources: Arc::new(crate::test_support::FixtureResourceResolver),
        models: crate::test_support::fixed_model(driver.clone()),
        capability_gateway: Arc::new(StaticCapabilityGateway::new(result.clone())),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        workflows: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
        approvals: store.clone(),
        clock: Arc::new(FixedClock::new(1_000)),
        ids: Arc::new(SequentialIds::default()),
    })
    .unwrap();
    let session = host
        .create_session(
            PrincipalId::try_new("user").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project".into()),
                ProjectSessionId::new("project-session"),
            ),
        )
        .await
        .unwrap();
    let question = |index| {
        format!(
            "question-{index}:{}:question-tail-{index}",
            "q".repeat(2048)
        )
    };
    for index in 0..12 {
        let outcome = host
            .submit_turn(&session.id, &session.project, question(index), vec![], None)
            .await;
        assert_eq!(outcome.is_err(), index == 1 || index == 2);
    }
    let requests = driver.0.lock().unwrap();
    assert_eq!(requests.len(), 12);
    let skills = SkillRegistry::with_builtins().unwrap();
    let report = skills
        .resolve_exact(
            &SkillId::try_new(STATISTICAL_REPORT_WRITING_ID).unwrap(),
            &SkillVersion::try_new(STATISTICAL_REPORT_WRITING_VERSION).unwrap(),
        )
        .unwrap();
    for request in requests.iter() {
        let skill_messages = request
            .messages
            .iter()
            .enumerate()
            .filter_map(|(index, message)| match message {
                AgentMessage::System { content } if content.ends_with(&report.instructions) => {
                    Some((index, content))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(skill_messages.len(), 1);
        let (skill_index, content) = skill_messages[0];
        assert!(content.contains(STATISTICAL_REPORT_WRITING_ID));
        assert!(content.contains(STATISTICAL_REPORT_WRITING_VERSION));
        assert!(!content.contains(report.manifest.source_hash.as_str()));
        assert!(skill_index > 0);
        assert!(
            skill_index
                < request
                    .messages
                    .iter()
                    .position(|message| matches!(message, AgentMessage::User { .. }))
                    .unwrap()
        );
        assert_eq!(
            request.tools,
            ToolRegistry::for_agent(yss_harness_contract::AgentRole::Manager).descriptors()
        );
    }
    let messages = &requests[11].messages;
    let users = messages
        .iter()
        .filter_map(|message| match message {
            AgentMessage::User { content } => Some(content.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(users, (0..12).map(question).collect::<Vec<_>>());
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(message, AgentMessage::ToolCall { .. }))
            .count(),
        22
    );
    assert_eq!(messages.iter().filter(|message| matches!(message, AgentMessage::ToolResult { outcome: Ok(value), .. } if value == &result)).count(), 11);
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(
                message,
                AgentMessage::ToolResult {
                    outcome: Err(_),
                    ..
                }
            ))
            .count(),
        11
    );
    let answers = messages
        .iter()
        .filter_map(|message| match message {
            AgentMessage::Assistant { content } => Some(content),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        answers
            .iter()
            .filter(|text| text.contains(":answer-tail-0"))
            .count(),
        1
    );
    assert!(
        answers
            .iter()
            .any(|text| text.contains("[Harness turn status: cancelled"))
    );
    assert!(
        answers
            .iter()
            .any(|text| text.contains("[Harness turn status: failed"))
    );
    for pair in messages.windows(2) {
        if let AgentMessage::ToolCall { invocation_id, .. } = &pair[0] {
            assert!(
                matches!(&pair[1], AgentMessage::ToolResult { invocation_id: result_id, .. } if result_id == invocation_id)
            );
        }
    }
}

#[tokio::test]
async fn startup_recovery_closes_interrupted_read_only_tools_and_turns_once() {
    use yss_harness_contract::{
        IdempotencyKey, InspectDatasetProfileRequest, ToolInvocationId, ToolInvocationRecord,
        ToolInvocationState, UnixMillis,
    };
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = HarnessHost::new(HarnessPorts {
        resources: Arc::new(crate::test_support::FixtureResourceResolver),
        models: crate::test_support::fixed_model(Arc::new(MockAgentDriver::new("Ready"))),
        capability_gateway: Arc::new(RejectingCapabilityGateway),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        workflows: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
        approvals: store.clone(),
        clock: Arc::new(FixedClock::new(1000)),
        ids: Arc::new(SequentialIds::default()),
    })
    .unwrap();
    let session = host
        .create_session(
            PrincipalId::try_new("user-1").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-1"),
            ),
        )
        .await
        .unwrap();
    let turn = HarnessTurnRecord {
        id: HarnessTurnId::try_new("turn-interrupted").unwrap(),
        session_id: session.id.clone(),
        state: HarnessTurnState::Running,
        user_message: "Profile".into(),
        final_text: None,
        started_at: UnixMillis::from_existing(100),
        finished_at: None,
    };
    store.create_turn(&turn).await.unwrap();
    store
        .begin(&ToolInvocationRecord {
            agent_run_id: None,
            id: ToolInvocationId::try_new("tool-interrupted").unwrap(),
            idempotency_key: IdempotencyKey::try_new("tool-interrupted").unwrap(),
            session_id: session.id.clone(),
            turn_id: turn.id.clone(),
            workflow_run_id: None,
            workflow_step_id: None,
            project: session.project,
            capability_id: yss_harness_contract::CapabilityId::InspectDatasetProfile,
            request: AutomationCapabilityRequest::InspectDatasetProfile(
                InspectDatasetProfileRequest {
                    database_id: "data-1".into(),
                },
            ),
            state: ToolInvocationState::Running,
            result: None,
            failure: None,
            started_at: UnixMillis::from_existing(100),
            deadline: UnixMillis::from_existing(200),
            finished_at: None,
        })
        .await
        .unwrap();
    assert_eq!(host.recover_interrupted_turns().await.unwrap(), 1);
    assert_eq!(host.recover_interrupted_turns().await.unwrap(), 0);
    assert!(store.load_running_invocations().await.unwrap().is_empty());
    assert!(store.load_running_turns().await.unwrap().is_empty());
    assert_eq!(
        store.load_turn(&turn.id).await.unwrap().unwrap().state,
        HarnessTurnState::Failed
    );
    assert!(matches!(
        host.events_after(&session.id, 1).await.unwrap().as_slice(),
        [
            HarnessEventEnvelope {
                event: HarnessEvent::Agent(AgentEvent::ToolInvocationFailed { .. }),
                ..
            },
            HarnessEventEnvelope {
                event: HarnessEvent::TurnFailed,
                ..
            },
        ]
    ));
}

#[tokio::test]
async fn dataset_quality_workflow_persists_and_completes_its_typed_tool_step() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = HarnessHost::new(HarnessPorts {
        resources: Arc::new(crate::test_support::FixtureResourceResolver),
        models: crate::test_support::fixed_model(Arc::new(MockAgentDriver::new("Plan ready."))),
        capability_gateway: Arc::new(
            StaticCapabilityGateway::new(AutomationCapabilityResult::DatasetSchemaInspection(
                DatasetSchemaInspection {
                    database_id: "database-1".to_owned(),
                    runtime_revision: 1,
                    schema_revision: 1,
                    columns: Vec::new(),
                },
            ))
            .with_result(AutomationCapabilityResult::DatasetProfileInspection(
                DatasetProfileInspection {
                    database_id: "database-1".to_owned(),
                    runtime_revision: 1,
                    schema_revision: 1,
                    row_count: 0,
                    column_count: 0,
                    estimated_memory_bytes: Some(0),
                    duplicated_rows: Some(0),
                    numeric_columns: 0,
                    categorical_columns: 0,
                    string_columns: 0,
                    temporal_columns: 0,
                    boolean_columns: 0,
                    total_nulls: 0,
                    null_ratio: 0.0,
                    columns_with_nulls: 0,
                    rows_with_nulls: 0,
                },
            )),
        ),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        workflows: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
        approvals: store.clone(),
        clock: Arc::new(FixedClock::new(2_000)),
        ids: Arc::new(SequentialIds::default()),
    })
    .unwrap();
    let session = host
        .create_session(
            PrincipalId::try_new("user-1").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-1"),
            ),
        )
        .await
        .unwrap();
    host.submit_turn(
        &session.id,
        &session.project,
        "Review quality.".to_owned(),
        vec![],
        None,
    )
    .await
    .unwrap();
    let turn_id = host.events_after(&session.id, 0).await.unwrap()[1]
        .turn_id
        .clone()
        .unwrap();
    let schema_step = WorkflowStepId::try_new("inspect_dataset_schema").unwrap();
    let workflow = crate::CompiledWorkflow::compile(WorkflowDefinition {
        id: WorkflowId::try_new("quality-review-test").unwrap(),
        version: WorkflowVersion::try_new("1.0.0").unwrap(),
        steps: vec![
            WorkflowStep {
                id: schema_step.clone(),
                depends_on: Vec::new(),
                request: AutomationCapabilityRequest::InspectDatasetSchema(
                    InspectDatasetSchemaRequest {
                        database_id: "database-1".into(),
                    },
                ),
            },
            WorkflowStep {
                id: WorkflowStepId::try_new("inspect_dataset_profile").unwrap(),
                depends_on: vec![schema_step],
                request: AutomationCapabilityRequest::InspectDatasetProfile(
                    InspectDatasetProfileRequest {
                        database_id: "database-1".into(),
                    },
                ),
            },
        ],
    })
    .unwrap();
    let planned = host
        .plan_workflow(&session.id, Some(&turn_id), &workflow)
        .await
        .unwrap();

    let running = host.advance_workflow(&planned.id).await.unwrap();
    assert_eq!(running.state, WorkflowRunState::Running);
    let completed = host.advance_workflow(&planned.id).await.unwrap();

    assert_eq!(completed.state, WorkflowRunState::Completed);
    assert!(
        host.events_after(&session.id, 0)
            .await
            .unwrap()
            .iter()
            .any(|event| matches!(event.event, HarnessEvent::WorkflowCompleted { .. }))
    );
}

#[tokio::test]
async fn project_session_reconciliation_stales_old_active_sessions() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = HarnessHost::new(HarnessPorts {
        resources: Arc::new(crate::test_support::FixtureResourceResolver),
        models: crate::test_support::fixed_model(Arc::new(MockAgentDriver::new("unused"))),
        capability_gateway: Arc::new(RejectingCapabilityGateway),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        workflows: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
        approvals: store.clone(),
        clock: Arc::new(FixedClock::new(3_000)),
        ids: Arc::new(SequentialIds::default()),
    })
    .unwrap();
    let old = host
        .create_session(
            PrincipalId::try_new("user-1").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-old"),
            ),
        )
        .await
        .unwrap();
    let current = ProjectSessionBinding::new(
        ProjectInstanceId::from_existing("project-1".into()),
        ProjectSessionId::new("project-session-current"),
    );

    assert_eq!(
        host.session_access()
            .await
            .reconcile_project_session(&current)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store.load_session(&old.id).await.unwrap().unwrap().state,
        HarnessSessionState::Stale
    );
}

struct ApprovedGateway;

impl CapabilityGatewayPort for ApprovedGateway {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        _control: yss_harness_contract::CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            assert!(context.approval_grant_id().is_some());
            assert!(matches!(
                request,
                AutomationCapabilityRequest::ApplyGraphEdit(_)
            ));
            Ok(AutomationCapabilityResult::GraphEditReceipt(
                GraphEditReceipt {
                    graph_hash: "0".repeat(64),
                    created_nodes: BTreeMap::new(),
                    created_ports: BTreeMap::new(),
                    graph_path: "events/Main.yssbi-event".to_owned(),
                    from_revision: 1,
                    to_revision: 2,
                    client_key: "assistant-edit-1".to_owned(),
                    changes: yss_harness_contract::GraphEditChanges {
                        base_semantic_input_hash: "0".repeat(64),
                        semantic_input_hash: "1".repeat(64),
                        nodes: Vec::new(),
                        removed_node_ids: Vec::new(),
                        connections: Vec::new(),
                        removed_connection_ids: Vec::new(),
                        constants: BTreeMap::new(),
                        removed_constant_ids: Vec::new(),
                        ready: true,
                        diagnostics: Vec::new(),
                    },
                },
            ))
        })
    }
}

#[tokio::test]
async fn approved_capability_is_ledgered_and_cannot_reuse_its_grant() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = HarnessHost::new(HarnessPorts {
        resources: Arc::new(crate::test_support::FixtureResourceResolver),
        models: crate::test_support::fixed_model(Arc::new(MockAgentDriver::new(
            "Ready for approval.",
        ))),
        capability_gateway: Arc::new(ApprovedGateway),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        workflows: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
        approvals: store.clone(),
        clock: Arc::new(FixedClock::new(4_000)),
        ids: Arc::new(SequentialIds::default()),
    })
    .unwrap();
    let session = host
        .create_session(
            PrincipalId::try_new("user-1").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-1"),
            ),
        )
        .await
        .unwrap();
    host.submit_turn(
        &session.id,
        &session.project,
        "Move the node.".to_owned(),
        vec![],
        None,
    )
    .await
    .unwrap();
    let turn_id = host.events_after(&session.id, 0).await.unwrap()[1]
        .turn_id
        .clone()
        .unwrap();
    let request = AutomationCapabilityRequest::ApplyGraphEdit(ApplyGraphEditRequest {
        graph_hash: "0".repeat(64),
        graph_path: "events/Main.yssbi-event".to_owned(),
        base_revision: 1,
        client_key: "assistant-edit-1".to_owned(),
        locale: "en-US".to_owned(),
        operations: vec![GraphEditOperation::MoveNodes {
            positions: vec![GraphEditPosition {
                node_id: "00000000-0000-0000-0000-000000000001".to_owned(),
                x: 10.0,
                y: 20.0,
            }],
        }],
    });
    let grant = host
        .issue_capability_approval(&session.id, &turn_id, &request, 1_000)
        .await
        .unwrap();

    assert!(matches!(
        host.execute_approved_capability(&session.id, &turn_id, &grant.id, request.clone())
            .await
            .unwrap(),
        AutomationCapabilityResult::GraphEditReceipt(_)
    ));
    assert!(matches!(
        host.execute_approved_capability(&session.id, &turn_id, &grant.id, request)
            .await,
        Err(HarnessError::Approval(ApprovalError::AlreadyConsumed))
    ));
}
