use super::*;
use crate::HarnessHost;
use crate::test_support::{
    InMemoryHarnessStore, NoteExtension, READ_NOTE, ScriptedDriver, ScriptedStep, WRITE_NOTE,
    note_ports, project,
};
use serde_json::json;

fn task(worker: &str, notes: &[&str], write: bool) -> Value {
    json!({
        "worker": worker,
        "objective": format!("Handle {notes:?}"),
        "constraints": "Preserve other notes",
        "completionCriteria": "Report the result",
        "dependsOn": [],
        "scope": {"notes": notes, "write": write},
    })
}

async fn run(
    extension: NoteExtension,
    driver: Arc<ScriptedDriver>,
    messages: &[&str],
    options: HarnessTurnOptions,
) -> (HarnessHost, Arc<InMemoryHarnessStore>, HarnessSessionRecord) {
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = HarnessHost::new(note_ports(
        store.clone(),
        crate::test_support::fixed_model(driver),
        extension,
        1_000,
    ))
    .unwrap();
    let session = host
        .create_session(PrincipalId::try_new("user").unwrap(), project())
        .await
        .unwrap();
    for message in messages {
        host.submit_turn(
            &session.id,
            &session.project,
            (*message).into(),
            vec![],
            None,
            options,
        )
        .await
        .unwrap();
    }
    (host, store, session)
}

fn outcomes(driver: &ScriptedDriver, tool: &str) -> Vec<Value> {
    driver
        .outcomes()
        .into_iter()
        .filter(|(_, name, _)| name == tool)
        .map(|(_, _, outcome)| outcome.output)
        .collect()
}

fn finished(store: &InMemoryHarnessStore) -> Vec<AgentTaskOutcome> {
    store
        .published_events()
        .into_iter()
        .filter_map(|event| match event.event {
            HarnessEvent::AgentRunFinished { outcome } => Some(*outcome),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn identical_delegations_run_once_and_return_receipt_facts() {
    let delegate = task("helper", &["a"], true);
    let driver = Arc::new(
        ScriptedDriver::default()
            .script(
                "lead",
                vec![
                    ScriptedStep::Call(DELEGATE_TASK_TOOL, delegate.clone()),
                    ScriptedStep::Call(DELEGATE_TASK_TOOL, delegate),
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
    let extension = NoteExtension::default().with_note("a", "alpha");
    let (_host, store, _session) = run(
        extension.clone(),
        driver.clone(),
        &["Rewrite a"],
        Default::default(),
    )
    .await;
    let delegations = outcomes(&driver, DELEGATE_TASK_TOOL);
    assert_eq!(delegations.len(), 2);
    assert_eq!(delegations[0], delegations[1]);
    assert_eq!(delegations[0]["state"], "completed");
    assert_eq!(delegations[0]["changed"], json!(["a"]));
    assert_eq!(extension.executions(), 1);
    assert_eq!(extension.note("a").unwrap(), (2, "rewritten".into()));
    let worker = finished(&store)
        .into_iter()
        .find(|outcome| outcome.role.as_str() == "helper")
        .unwrap();
    assert_eq!(worker.evidence.len(), 1);
    assert_eq!(worker.facts, json!({"changed": ["a"]}));
    assert_eq!(worker.report.unwrap().summary, "done");
    let requests = driver.requests();
    let helper = requests
        .iter()
        .find(|request| request.role.as_str() == "helper")
        .unwrap();
    assert!(helper.tools.iter().any(|tool| tool.name == WRITE_NOTE));
    assert!(
        !helper
            .tools
            .iter()
            .any(|tool| tool.name == DELEGATE_TASK_TOOL)
    );
    assert!(
        matches!(&helper.messages[1], AgentMessage::System { content } if content.starts_with("Return a concise final message"))
    );
}

#[tokio::test]
async fn writes_invalidate_completed_readers_and_their_dependents() {
    let driver = Arc::new(
        ScriptedDriver::default()
            .script(
                "lead",
                vec![
                    ScriptedStep::Call(DELEGATE_TASK_TOOL, task("reader", &["a"], false)),
                    ScriptedStep::Call(DELEGATE_TASK_TOOL, task("helper", &["a"], true)),
                ],
            )
            .script(
                "reader",
                vec![ScriptedStep::Call(READ_NOTE, json!({"id": "a"}))],
            )
            .script(
                "helper",
                vec![ScriptedStep::Call(
                    WRITE_NOTE,
                    json!({"id": "a", "text": "changed"}),
                )],
            ),
    );
    let (_host, store, _session) = run(
        NoteExtension::default().with_note("a", "alpha"),
        driver.clone(),
        &["Read then write"],
        Default::default(),
    )
    .await;
    let delegations = outcomes(&driver, DELEGATE_TASK_TOOL);
    let reader = delegations[0]["runId"].clone();
    assert_eq!(delegations[1]["invalidatedRuns"], json!([reader]));
    assert!(store.published_events().iter().any(|event| matches!(
        &event.event,
        HarnessEvent::AgentRunInvalidated { run_id } if json!(run_id) == reader
    )));
}

#[tokio::test]
async fn dependencies_must_be_completed_in_the_turn() {
    let mut dependent = task("reader", &["b"], false);
    dependent["dependsOn"] = json!(["agent-404"]);
    let driver = Arc::new(ScriptedDriver::default().script(
        "lead",
        vec![ScriptedStep::Call(DELEGATE_TASK_TOOL, dependent)],
    ));
    run(
        NoteExtension::default(),
        driver.clone(),
        &["Depend"],
        Default::default(),
    )
    .await;
    let output = &outcomes(&driver, DELEGATE_TASK_TOOL)[0];
    assert_eq!(output["state"], "failed");
    assert_eq!(
        output["failure"]["details"]["reason"],
        "dependency_not_completed"
    );
}

#[tokio::test]
async fn ask_mode_rejects_write_tasks_and_filters_mutating_tools() {
    let driver = Arc::new(
        ScriptedDriver::default()
            .script(
                "lead",
                vec![
                    ScriptedStep::Call(DELEGATE_TASK_TOOL, task("helper", &["a"], true)),
                    ScriptedStep::Call(DELEGATE_TASK_TOOL, task("helper", &["a"], false)),
                ],
            )
            .script(
                "helper",
                vec![ScriptedStep::Call(
                    WRITE_NOTE,
                    json!({"id": "a", "text": "changed"}),
                )],
            ),
    );
    let extension = NoteExtension::default().with_note("a", "alpha");
    run(
        extension.clone(),
        driver.clone(),
        &["Inspect only"],
        HarnessTurnOptions {
            mode: HarnessMode::Ask,
            reasoning_effort: None,
        },
    )
    .await;
    let delegations = outcomes(&driver, DELEGATE_TASK_TOOL);
    assert_eq!(
        delegations[0]["failure"]["details"]["reason"],
        "ask_mode_read_only"
    );
    assert_eq!(delegations[1]["state"], "completed");
    let requests = driver.requests();
    let helper = requests
        .iter()
        .find(|request| request.role.as_str() == "helper")
        .unwrap();
    assert!(!helper.tools.iter().any(|tool| tool.name == WRITE_NOTE));
    assert!(
        matches!(&helper.messages[0], AgentMessage::System { content } if content == "Ask mode.")
    );
    let rejected = outcomes(&driver, WRITE_NOTE);
    assert_eq!(rejected[0]["state"], "failed");
    assert_eq!(extension.executions(), 0);
}

#[tokio::test]
async fn workers_cannot_delegate_and_out_of_scope_calls_keep_only_decoded_arguments() {
    let driver = Arc::new(
        ScriptedDriver::default()
            .script(
                "lead",
                vec![ScriptedStep::Call(
                    DELEGATE_TASK_TOOL,
                    task("helper", &["a"], true),
                )],
            )
            .script(
                "helper",
                vec![
                    ScriptedStep::Call(DELEGATE_TASK_TOOL, task("reader", &["a"], false)),
                    ScriptedStep::Call(WRITE_NOTE, json!({"id": "b", "text": "outside"})),
                    ScriptedStep::Call(WRITE_NOTE, json!({"text": "malformed"})),
                ],
            ),
    );
    let extension = NoteExtension::default().with_note("a", "alpha");
    let (_host, store, _session) = run(
        extension.clone(),
        driver.clone(),
        &["Rewrite a"],
        Default::default(),
    )
    .await;
    let nested = &outcomes(&driver, DELEGATE_TASK_TOOL)[0];
    assert_eq!(
        nested["failure"]["details"]["reason"],
        "workers_cannot_delegate"
    );
    let writes = outcomes(&driver, WRITE_NOTE);
    assert_eq!(writes[0]["failure"]["details"]["reason"], "scope_denied");
    assert_eq!(writes[1]["state"], "failed");
    let records: Vec<_> = store
        .tool_invocations()
        .into_iter()
        .filter(|record| record.tool == WRITE_NOTE)
        .collect();
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|record| record.request.is_none()));
    assert!(records.iter().any(|record| record.arguments["id"] == "b"));
    assert!(records.iter().any(|record| record.arguments == json!({})));
    assert_eq!(extension.executions(), 0);
}

#[tokio::test]
async fn idempotent_calls_reuse_receipts_and_recover_unknown_outcomes() {
    let write = json!({"id": "a", "text": "once", "key": "client-1"});
    let driver = Arc::new(
        ScriptedDriver::default()
            .script(
                "lead",
                vec![ScriptedStep::Call(
                    DELEGATE_TASK_TOOL,
                    task("helper", &["a"], true),
                )],
            )
            .script(
                "helper",
                vec![
                    ScriptedStep::Call(WRITE_NOTE, write.clone()),
                    ScriptedStep::Call(WRITE_NOTE, write.clone()),
                    ScriptedStep::Call(
                        WRITE_NOTE,
                        json!({"id": "a", "text": "changed", "key": "client-1"}),
                    ),
                ],
            ),
    );
    let extension = NoteExtension::default().with_note("a", "alpha");
    extension.fail_next(CapabilityFailure::new(
        CapabilityFailureCode::OUTCOME_UNKNOWN,
    ));
    extension.recover_with(ToolRecovery::Committed(json!({"id": "a", "revision": 7})));
    run(
        extension.clone(),
        driver.clone(),
        &["Write once"],
        Default::default(),
    )
    .await;
    let writes = outcomes(&driver, WRITE_NOTE);
    assert_eq!(writes[0]["state"], "failed");
    assert_eq!(writes[0]["failure"]["code"], "outcome_unknown");
    // The retry finds the unknown invocation and asks the owner, never repeating the write.
    assert_eq!(writes[1], json!({"id": "a", "revision": 7}));
    assert_eq!(writes[2]["failure"]["code"], "invocation_conflict");
    assert_eq!(extension.executions(), 1);
}

#[tokio::test]
async fn followup_resumes_the_same_worker_with_its_history_and_receipts() {
    let driver = Arc::new(
        ScriptedDriver::default()
            .script(
                "lead",
                vec![ScriptedStep::Call(
                    DELEGATE_TASK_TOOL,
                    task("helper", &["a"], true),
                )],
            )
            .script(
                "helper",
                vec![
                    ScriptedStep::Text("Started."),
                    ScriptedStep::Call(WRITE_NOTE, json!({"id": "a", "text": "draft"})),
                ],
            ),
    );
    let extension = NoteExtension::default().with_note("a", "alpha");
    let (host, store, session) = run(
        extension.clone(),
        driver.clone(),
        &["Draft a"],
        Default::default(),
    )
    .await;
    let run_id = outcomes(&driver, DELEGATE_TASK_TOOL)[0]["runId"].clone();
    driver.push(
        "lead",
        vec![ScriptedStep::Call(
            FOLLOWUP_TASK_TOOL,
            json!({"runId": run_id, "instruction": "Finish the note"}),
        )],
    );
    driver.push(
        "helper",
        vec![ScriptedStep::Call(
            WRITE_NOTE,
            json!({"id": "a", "text": "final"}),
        )],
    );
    host.submit_turn(
        &session.id,
        &session.project,
        "Finish a".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
    let followup = &outcomes(&driver, FOLLOWUP_TASK_TOOL)[0];
    assert_eq!(followup["runId"], run_id);
    assert_eq!(followup["state"], "completed");
    assert_eq!(followup["changed"], json!(["a", "a"]));
    assert_eq!(extension.note("a").unwrap().1, "final");
    assert!(store.published_events().iter().any(|event| matches!(
        &event.event,
        HarnessEvent::AgentRunResumed { request, scope, .. }
            if json!(request.run_id) == run_id
                && request.bindings == json!({"rebound": true})
                && scope == &json!({"notes": ["a"], "write": true})
    )));
    let requests = driver.requests();
    let resumed = requests
        .iter()
        .filter(|request| request.role.as_str() == "helper")
        .nth(1)
        .unwrap();
    assert!(resumed.messages.iter().any(|message| matches!(message, AgentMessage::ToolCall { tool, arguments, .. } if tool == WRITE_NOTE && arguments["text"] == "draft")));
    assert!(resumed.messages.iter().any(
        |message| matches!(message, AgentMessage::Assistant { content } if content == "Started.")
    ));
    assert!(resumed.messages.iter().any(
        |message| matches!(message, AgentMessage::User { content } if content == "Finish the note")
    ));
    assert!(
        matches!(resumed.messages.last(), Some(AgentMessage::User { content }) if content.contains("\"task\""))
    );
    let lead = requests
        .iter()
        .filter(|request| request.role.as_str() == "lead")
        .nth(1)
        .unwrap();
    // The lead replays the delegation as its own tool call, not the worker's private progress.
    assert!(lead.messages.iter().any(|message| matches!(message, AgentMessage::ToolCall { tool, arguments, .. } if tool == DELEGATE_TASK_TOOL && arguments["scope"] == json!({"notes": ["a"], "write": true}))));
    assert!(!lead.messages.iter().any(|message| matches!(message, AgentMessage::Assistant { content } if content.contains("Started."))));
}

#[tokio::test]
async fn startup_recovery_finishes_unsettled_runs_from_their_receipts() {
    let driver = Arc::new(ScriptedDriver::default());
    let extension = NoteExtension::default().with_note("a", "alpha");
    let (host, store, session) = run(extension.clone(), driver, &[], Default::default()).await;
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
    let run_id = AgentRunId::try_new("agent-worker").unwrap();
    let task = AgentTask {
        key: "task".into(),
        worker: crate::test_support::role("helper"),
        objective: "Write".into(),
        constraints: String::new(),
        completion_criteria: "Saved".into(),
        depends_on: vec![],
        scope: json!({"notes": ["a"], "write": true}),
    };
    for event in [
        HarnessEvent::AgentRunStarted {
            run_id: AgentRunId::try_new("agent-root").unwrap(),
            parent_run_id: None,
            role: crate::test_support::role("lead"),
            task: None,
        },
        HarnessEvent::AgentRunStarted {
            run_id: run_id.clone(),
            parent_run_id: Some(AgentRunId::try_new("agent-root").unwrap()),
            role: task.worker.clone(),
            task: Some(Box::new(task)),
        },
        HarnessEvent::AgentRunOutput {
            run_id: run_id.clone(),
            event: AgentEvent::ToolInvocationCompleted {
                invocation_id: ToolInvocationId::try_new("committed").unwrap(),
                tool: WRITE_NOTE.into(),
            },
        },
    ] {
        store
            .append_event(
                &session.id,
                Some(&turn.id),
                UnixMillis::from_existing(100),
                event,
            )
            .await
            .unwrap();
    }
    store
        .begin(&ToolInvocationRecord {
            id: ToolInvocationId::try_new("committed").unwrap(),
            idempotency_key: IdempotencyKey::try_new("committed").unwrap(),
            session_id: session.id.clone(),
            turn_id: turn.id.clone(),
            agent_run_id: Some(run_id.clone()),
            workflow_run_id: None,
            workflow_step_id: None,
            project: session.project.clone(),
            tool: WRITE_NOTE.into(),
            effect: ToolEffect::Mutate,
            arguments: json!({"id": "a", "text": "kept"}),
            request: Some(json!({"id": "a", "text": "kept"})),
            state: ToolInvocationState::Succeeded,
            result: Some(json!({"id": "a", "revision": 2})),
            failure: None,
            started_at: UnixMillis::from_existing(100),
            deadline: UnixMillis::from_existing(200),
            finished_at: Some(UnixMillis::from_existing(150)),
        })
        .await
        .unwrap();
    assert_eq!(host.recover_interrupted_turns().await.unwrap(), 1);
    let outcomes = finished(&store);
    let worker = outcomes
        .iter()
        .find(|outcome| outcome.run_id == run_id)
        .unwrap();
    assert_eq!(worker.state, AgentRunState::Interrupted);
    assert_eq!(worker.facts, json!({"changed": ["a"]}));
    assert_eq!(worker.evidence.len(), 1);
    assert_eq!(
        worker.report.as_ref().unwrap().blocked_reason.as_deref(),
        Some("interrupted")
    );
    let root = outcomes
        .iter()
        .find(|outcome| outcome.run_id.as_str() == "agent-root")
        .unwrap();
    assert!(root.report.is_none());
    // Startup recovery records the outcome; it never repeats a committed write.
    assert_eq!(extension.executions(), 0);
}

#[tokio::test]
async fn control_calls_close_after_decode_and_admission_failures() {
    let driver = Arc::new(ScriptedDriver::default().script(
        "lead",
        vec![
            ScriptedStep::Call(DELEGATE_TASK_TOOL, json!({"worker": 42})),
            ScriptedStep::Call(FOLLOWUP_TASK_TOOL, json!({"runId": 42})),
            ScriptedStep::Call(
                FOLLOWUP_TASK_TOOL,
                json!({"runId": "agent-404", "instruction": "Continue"}),
            ),
            ScriptedStep::Call(DELEGATE_TASK_TOOL, task("reader", &["a"], true)),
            ScriptedStep::Call(DELEGATE_TASK_TOOL, task("reader", &["a"], false)),
        ],
    ));
    let (_host, store, _session) = run(
        NoteExtension::default().with_note("a", "alpha"),
        driver.clone(),
        &["Coordinate"],
        Default::default(),
    )
    .await;
    let mut started = Vec::new();
    let mut finishes = Vec::new();
    for event in store.published_events() {
        match event.event {
            HarnessEvent::Agent(AgentEvent::ControlToolStarted {
                invocation_id,
                tool,
            }) => started.push((invocation_id, tool)),
            HarnessEvent::Agent(AgentEvent::ControlToolFinished {
                invocation_id,
                tool,
                failure_code,
                failure_details,
            }) => finishes.push((invocation_id, tool, failure_code, failure_details)),
            _ => {}
        }
    }
    assert_eq!(started.len(), 5);
    assert_eq!(finishes.len(), 5);
    for ((started, tool), (finished, finished_tool, _, _)) in started.iter().zip(&finishes) {
        assert_eq!((started, tool), (finished, finished_tool));
    }
    let codes: Vec<_> = finishes
        .iter()
        .map(|(_, _, code, _)| code.as_ref().map(|code| code.as_str().to_owned()))
        .collect();
    assert_eq!(
        codes,
        [
            Some("invalid_request".into()),
            Some("invalid_request".into()),
            Some("invalid_request".into()),
            Some("invalid_request".into()),
            None
        ]
    );
    let details = |index: usize| finishes[index].3.clone().unwrap_or_default();
    assert_eq!(details(1)["category"], "invalid_type");
    assert_eq!(details(1)["path"], "$.runId");
    assert_eq!(details(2)["reason"], "worker_not_found");
    assert_eq!(details(3)["reason"], "reader_cannot_write");
}
