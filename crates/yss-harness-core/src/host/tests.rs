use crate::{HarnessError, HarnessHost};
use yss_harness_contract::*;

use std::sync::{Arc, Mutex};

use super::*;
use crate::test_support::{
    InMemoryHarnessStore, MockAgentDriver, NoteExtension, READ_NOTE, ScriptedDriver, ScriptedStep,
    WRITE_NOTE, note_ports, project,
};
use serde_json::json;

fn host(store: &Arc<InMemoryHarnessStore>, driver: Arc<dyn AgentDriverPort>) -> HarnessHost {
    HarnessHost::new(note_ports(
        store.clone(),
        crate::test_support::fixed_model(driver),
        NoteExtension::default().with_note("a", "alpha"),
        1_000,
    ))
    .unwrap()
}

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
    let host = Arc::new(host(&store, Arc::new(MockAgentDriver::new("unused"))));
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
    let host = host(&store, Arc::new(MockAgentDriver::new("Evidence is ready.")));
    let session = host
        .create_session(PrincipalId::try_new("user-1").unwrap(), project())
        .await
        .unwrap();

    let references = vec![HarnessResourceReference {
        resource: ProjectResourceRef {
            kind: ProjectResourceKind::Database,
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
            Default::default(),
        )
        .await
        .unwrap();
    let events = host.events_after(&session.id, 0).await.unwrap();

    assert_eq!(result.final_text, "Evidence is ready.");
    assert!(events.iter().any(|event| matches!(&event.event,
        HarnessEvent::AgentRunFinished { outcome }
            if outcome.role.as_str() == "lead"
                && outcome.state == AgentRunState::Completed
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
    let host = host(&store, Arc::new(MockAgentDriver::new("unused")));
    let principal = PrincipalId::try_new("user").unwrap();
    let session = host
        .session_access()
        .await
        .create_conversation(principal.clone(), "project-key".into(), project())
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
    struct RecordingDriver(Mutex<Vec<AgentTurnRequest>>);
    impl AgentDriverPort for RecordingDriver {
        fn run_turn<'a>(
            &'a self,
            request: AgentTurnRequest,
            tools: Arc<dyn AgentToolExecutor>,
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
                let read = tools
                    .call(ToolCall {
                        name: READ_NOTE.into(),
                        arguments: json!({"id": "a"}),
                    })
                    .await;
                assert_eq!(read.output["text"], "alpha");
                // The lead has no write tool; a provider call to it is still ledgered as rejected.
                let rejected = tools
                    .call(ToolCall {
                        name: WRITE_NOTE.into(),
                        arguments: json!({"id": "a", "text": "changed"}),
                    })
                    .await;
                assert_eq!(rejected.output["state"], "failed");
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
    let host = host(&store, driver.clone());
    let session = host
        .create_session(PrincipalId::try_new("user").unwrap(), project())
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
            .submit_turn(
                &session.id,
                &session.project,
                question(index),
                vec![],
                None,
                Default::default(),
            )
            .await;
        assert_eq!(outcome.is_err(), index == 1 || index == 2);
    }
    let requests = driver.0.lock().unwrap();
    assert_eq!(requests.len(), 12);
    let lead = crate::test_support::role("lead");
    for request in requests.iter() {
        assert_eq!(request.role, lead);
        assert!(
            matches!(&request.messages[0], AgentMessage::System { content } if content == "You are the lead.")
        );
        assert_eq!(
            request
                .tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            [
                READ_KNOWLEDGE_TOOL,
                READ_NOTE,
                SEARCH_KNOWLEDGE_TOOL,
                DELEGATE_TASK_TOOL,
                FOLLOWUP_TASK_TOOL
            ]
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
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(message, AgentMessage::ToolResult { output, .. } if output["text"] == "alpha"))
            .count(),
        11
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(message, AgentMessage::ToolResult { output, .. } if output["state"] == "failed"))
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
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = host(&store, Arc::new(MockAgentDriver::new("Ready")));
    let session = host
        .create_session(PrincipalId::try_new("user-1").unwrap(), project())
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
    let read = ToolInvocationRecord {
        agent_run_id: None,
        id: ToolInvocationId::try_new("tool-interrupted").unwrap(),
        idempotency_key: IdempotencyKey::try_new("tool-interrupted").unwrap(),
        session_id: session.id.clone(),
        turn_id: turn.id.clone(),
        project: session.project.clone(),
        tool: READ_NOTE.into(),
        effect: ToolEffect::Inspect,
        arguments: json!({"id": "a"}),
        request: Some(json!({"id": "a"})),
        state: ToolInvocationState::Running,
        result: None,
        failure: None,
        started_at: UnixMillis::from_existing(100),
        deadline: UnixMillis::from_existing(200),
        finished_at: None,
    };
    store.begin(&read).await.unwrap();
    let rejected = ToolInvocationRecord {
        id: ToolInvocationId::try_new("rejected-write-interrupted").unwrap(),
        idempotency_key: IdempotencyKey::try_new("rejected-write-interrupted").unwrap(),
        tool: WRITE_NOTE.into(),
        effect: ToolEffect::Mutate,
        request: None,
        ..read
    };
    store.begin(&rejected).await.unwrap();
    assert_eq!(host.recover_interrupted_turns().await.unwrap(), 1);
    assert_eq!(host.recover_interrupted_turns().await.unwrap(), 0);
    assert!(store.load_running_invocations().await.unwrap().is_empty());
    assert!(store.load_running_turns().await.unwrap().is_empty());
    assert_eq!(
        store
            .load_invocation(&session.id, &rejected.id)
            .await
            .unwrap()
            .unwrap()
            .failure
            .unwrap()
            .code,
        CapabilityFailureCode::INTERNAL_FAILURE
    );
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
async fn interrupted_unbound_and_bound_writes_have_distinct_recovered_outcomes() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = host(&store, Arc::new(MockAgentDriver::new("Ready")));
    let session = host
        .create_session(PrincipalId::try_new("user-1").unwrap(), project())
        .await
        .unwrap();
    let turn = HarnessTurnRecord {
        id: HarnessTurnId::try_new("turn-interrupted").unwrap(),
        session_id: session.id.clone(),
        state: HarnessTurnState::Running,
        user_message: "Write".into(),
        final_text: None,
        started_at: UnixMillis::from_existing(100),
        finished_at: None,
    };
    store.create_turn(&turn).await.unwrap();
    store
        .begin(&ToolInvocationRecord {
            agent_run_id: None,
            id: ToolInvocationId::try_new("write-interrupted").unwrap(),
            idempotency_key: IdempotencyKey::try_new("write-interrupted").unwrap(),
            session_id: session.id.clone(),
            turn_id: turn.id.clone(),
            project: session.project.clone(),
            tool: WRITE_NOTE.into(),
            effect: ToolEffect::Mutate,
            arguments: json!({"id": "a", "text": "changed"}),
            request: Some(json!({"id": "a", "text": "changed"})),
            state: ToolInvocationState::Running,
            result: None,
            failure: None,
            started_at: UnixMillis::from_existing(100),
            deadline: UnixMillis::from_existing(200),
            finished_at: None,
        })
        .await
        .unwrap();
    host.recover_interrupted_turns().await.unwrap();
    // A bound mutation may already have committed; recovery must not imply that it rolled back.
    assert_eq!(
        store.tool_invocations()[0].failure.as_ref().unwrap().code,
        CapabilityFailureCode::OUTCOME_UNKNOWN
    );
}

#[tokio::test]
async fn project_session_reconciliation_stales_old_active_sessions() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = host(&store, Arc::new(MockAgentDriver::new("unused")));
    let old = host
        .create_session(
            PrincipalId::try_new("user-1").unwrap(),
            ProjectSessionBinding::new(
                yss_project_identity::ProjectInstanceId::from_existing("project-1".into()),
                yss_project_identity::ProjectSessionId::new("project-session-old"),
            ),
        )
        .await
        .unwrap();

    assert_eq!(
        host.session_access()
            .await
            .reconcile_project_session(&project())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store.load_session(&old.id).await.unwrap().unwrap().state,
        HarnessSessionState::Stale
    );
}

#[tokio::test]
async fn scripted_turn_publishes_extension_notices_after_the_settled_call() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let driver = Arc::new(
        ScriptedDriver::default()
            .script(
                "lead",
                vec![
                    ScriptedStep::Call(
                        DELEGATE_TASK_TOOL,
                        json!({
                            "worker": "helper", "objective": "Rewrite a", "constraints": "",
                            "completionCriteria": "Saved", "dependsOn": [],
                            "scope": {"notes": ["a"], "write": true}
                        }),
                    ),
                    ScriptedStep::Text("Delegated."),
                ],
            )
            .script(
                "helper",
                vec![ScriptedStep::Call(
                    WRITE_NOTE,
                    json!({"id": "a", "text": "rewritten"}),
                )],
            ),
    );
    let host = host(&store, driver.clone());
    let session = host
        .create_session(PrincipalId::try_new("user-1").unwrap(), project())
        .await
        .unwrap();
    host.submit_turn(
        &session.id,
        &session.project,
        "Rewrite a.".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
    let events = host.events_after(&session.id, 0).await.unwrap();
    let completed = events
        .iter()
        .position(|event| matches!(&event.event, HarnessEvent::AgentRunOutput { event: AgentEvent::ToolInvocationCompleted { tool, .. }, .. } if tool == WRITE_NOTE))
        .unwrap();
    assert!(matches!(&events[completed + 1].event,
        HarnessEvent::AgentRunOutput { event: AgentEvent::Custom { invocation_id: Some(_), kind, payload }, .. }
            if kind == "note_written" && payload["id"] == "a"));
    let outcome = driver
        .outcomes()
        .into_iter()
        .find(|(_, tool, _)| tool == DELEGATE_TASK_TOOL)
        .unwrap()
        .2;
    assert_eq!(outcome.output["state"], "completed");
    assert_eq!(outcome.output["changed"], json!(["a"]));
    assert_eq!(outcome.output["role"], "helper");
}
