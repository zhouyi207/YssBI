use super::*;
use crate::test_support::{
    FixedClock, InMemoryHarnessStore, NoteExtension, NoteGate, READ_NOTE, SequentialIds,
    WRITE_NOTE, note_specs, project,
};
use serde_json::json;
use std::sync::Mutex;
use yss_harness_contract::{AgentFuture, AgentOutputFailure, CancellationReason};

#[derive(Default)]
struct Output(Mutex<Vec<AgentEvent>>);

impl AgentEventOutput for Output {
    fn emit<'a>(&'a self, event: AgentEvent) -> AgentFuture<'a, Result<(), AgentOutputFailure>> {
        Box::pin(async move {
            self.0.lock().unwrap().push(event);
            Ok(())
        })
    }
}

struct Fixture {
    store: Arc<InMemoryHarnessStore>,
    clock: Arc<FixedClock>,
    output: Arc<Output>,
    cancellation: CancellationToken,
    executor: HarnessToolExecutor,
}

fn setup(extension: NoteExtension) -> Fixture {
    setup_with(extension, note_specs(true), Arc::default())
}

fn setup_with(
    extension: NoteExtension,
    specs: Vec<ToolSpec>,
    approvals: Arc<crate::approvals::Approvals>,
) -> Fixture {
    let store = Arc::new(InMemoryHarnessStore::default());
    let clock = Arc::new(FixedClock::new(1000));
    let output = Arc::new(Output::default());
    let cancellation = CancellationToken::default();
    let executor = HarnessToolExecutor::new(
        ToolRegistry::new(specs),
        ToolServices {
            extension: Arc::new(extension),
            knowledge: Arc::new(crate::KnowledgeService::new(
                store.clone(),
                Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
            )),
            web: Arc::new(yss_harness_contract::DisabledWebAccess),
            python: std::sync::Arc::new(yss_harness_contract::DisabledPython),
            approvals,
            ledger: store.clone(),
            clock: clock.clone(),
            ids: Arc::new(SequentialIds::default()),
        },
        InvocationScope {
            principal_id: PrincipalId::try_new("user-1").unwrap(),
            session_id: HarnessSessionId::try_new("session-1").unwrap(),
            turn_id: HarnessTurnId::try_new("turn-1").unwrap(),
            project: project(),
        },
        cancellation.clone(),
    )
    .with_output(output.clone());
    Fixture {
        store,
        clock,
        output,
        cancellation,
        executor,
    }
}

fn call(tool: &str, request: serde_json::Value) -> PreparedToolCall {
    PreparedToolCall {
        tool: tool.into(),
        arguments: request.clone(),
        request,
        idempotency_token: None,
        internal: false,
    }
}

async fn settle_with(
    fixture: &Fixture,
    gate: &NoteGate,
    action: impl FnOnce(&Fixture),
) -> Result<ToolInvocationOutcome, CapabilityFailure> {
    let execution = fixture
        .executor
        .execute(call(READ_NOTE, json!({"id": "a"})), fixture.executor.now());
    tokio::pin!(execution);
    tokio::select! {
        _ = gate.entered.notified() => {}
        _ = &mut execution => panic!("the owner call must be held"),
    }
    action(fixture);
    gate.release.notify_one();
    execution.await
}

fn assert_closed(fixture: &Fixture, expected: &CapabilityFailureCode) {
    let records = fixture.store.tool_invocations();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].state, ToolInvocationState::Failed);
    assert_eq!(&records[0].failure.as_ref().unwrap().code, expected);
    assert!(records[0].finished_at.is_some());
    let events = fixture.output.0.lock().unwrap();
    assert!(matches!(events.as_slice(), [
        AgentEvent::ToolInvocationStarted { invocation_id: started, .. },
        AgentEvent::ToolInvocationFailed { invocation_id: finished, failure_code, failure_details: Some(details), .. },
    ] if started == finished && failure_code == expected && details.contains_key("capabilityId")));
}

#[tokio::test]
async fn owner_failures_close_the_ledger_and_emit_the_identified_terminal_event() {
    let domain = CapabilityFailureCode::from_static("note_locked");
    let extension = NoteExtension::default().with_note("a", "alpha");
    extension.fail_next(CapabilityFailure::new(domain.clone()));
    let fixture = setup(extension);
    let failure = fixture
        .executor
        .execute(call(READ_NOTE, json!({"id": "a"})), fixture.executor.now())
        .await
        .unwrap_err();
    assert_eq!(failure.code, domain);
    assert_eq!(failure.details["capabilityId"], READ_NOTE);
    assert_closed(&fixture, &domain);

    let extension = NoteExtension::default().with_note("a", "alpha");
    extension.panic_next();
    let fixture = setup(extension);
    let failure = fixture
        .executor
        .execute(call(READ_NOTE, json!({"id": "a"})), fixture.executor.now())
        .await
        .unwrap_err();
    assert_eq!(failure.code, CapabilityFailureCode::INTERNAL_FAILURE);
    assert_closed(&fixture, &CapabilityFailureCode::INTERNAL_FAILURE);
}

#[tokio::test]
async fn late_owner_results_cannot_outlive_cancellation_or_the_deadline() {
    let (extension, gate) = NoteExtension::default().with_note("a", "alpha").with_gate();
    let fixture = setup(extension);
    let failure = settle_with(&fixture, &gate, |fixture| {
        fixture.cancellation.cancel(CancellationReason::User);
    })
    .await
    .unwrap_err();
    assert_eq!(failure.code, CapabilityFailureCode::CANCELLED);
    assert_closed(&fixture, &CapabilityFailureCode::CANCELLED);

    let (extension, gate) = NoteExtension::default().with_note("a", "alpha").with_gate();
    let fixture = setup(extension);
    let failure = settle_with(&fixture, &gate, |fixture| fixture.clock.advance(31_000))
        .await
        .unwrap_err();
    assert_eq!(failure.code, CapabilityFailureCode::DEADLINE_ELAPSED);
    assert_closed(&fixture, &CapabilityFailureCode::DEADLINE_ELAPSED);
}

#[tokio::test]
async fn committed_writes_are_reported_after_their_deadline() {
    let (extension, gate) = NoteExtension::default().with_note("a", "alpha").with_gate();
    let fixture = setup(extension.clone());
    let execution = fixture.executor.execute(
        call(WRITE_NOTE, json!({"id": "a", "text": "kept"})),
        fixture.executor.now(),
    );
    tokio::pin!(execution);
    tokio::select! {
        _ = gate.entered.notified() => {}
        _ = &mut execution => panic!("the owner call must be held"),
    }
    fixture.clock.advance(31_000);
    gate.release.notify_one();
    // A mutation's owner receipt is authoritative; elapsed time cannot hide the commit.
    let outcome = execution.await.unwrap();
    assert_eq!(outcome.result, json!({"id": "a", "revision": 2}));
    assert_eq!(extension.note("a").unwrap().1, "kept");
}

#[tokio::test]
async fn rejected_calls_keep_only_decoded_arguments_and_their_timing() {
    for (arguments, retained) in [
        (None, json!({})),
        (Some(json!({"id": "a"})), json!({"id": "a"})),
    ] {
        let fixture = setup(NoteExtension::default());
        let started_at = fixture.executor.now();
        fixture.clock.advance(37);
        let failure = fixture
            .executor
            .reject(
                WRITE_NOTE.into(),
                arguments,
                CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST),
                started_at,
            )
            .await
            .unwrap_err();
        assert_eq!(failure.code, CapabilityFailureCode::INVALID_REQUEST);
        assert_closed(&fixture, &CapabilityFailureCode::INVALID_REQUEST);
        let record = fixture.store.tool_invocations().remove(0);
        assert!(record.request.is_none());
        assert_eq!(record.arguments, retained);
        assert_eq!(record.effect, ToolEffect::Mutate);
        assert_eq!(
            record.finished_at.unwrap().get() - record.started_at.get(),
            37
        );
    }

    let fixture = setup(NoteExtension::default());
    let failure = fixture
        .executor
        .execute(
            call("unregistered", json!({"id": "a"})),
            fixture.executor.now(),
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code, CapabilityFailureCode::INVALID_REQUEST);
    assert_eq!(failure.details["capabilityId"], "unregistered");
}

fn executing_specs() -> Vec<ToolSpec> {
    note_specs(false)
        .into_iter()
        .map(|mut spec| {
            if spec.name == READ_NOTE {
                spec.effect = ToolEffect::Execute;
            }
            spec
        })
        .collect()
}

async fn answer(
    fixture: &Fixture,
    approvals: &crate::approvals::Approvals,
    decision: ApprovalDecision,
) -> Result<ToolInvocationOutcome, CapabilityFailure> {
    let execution = fixture
        .executor
        .execute(call(READ_NOTE, json!({"id": "a"})), fixture.executor.now());
    tokio::pin!(execution);
    loop {
        let requested =
            fixture
                .output
                .0
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find_map(|event| match event {
                    AgentEvent::ApprovalRequested { approval_id, .. } => Some(approval_id.clone()),
                    _ => None,
                });
        if let Some(approval_id) = requested
            && approvals.resolve(&approval_id, decision)
        {
            break;
        }
        tokio::select! {
            _ = tokio::task::yield_now() => {}
            outcome = &mut execution => return outcome,
        }
    }
    execution.await
}

#[tokio::test]
async fn executing_tools_wait_for_the_user_and_record_a_denial() {
    let approvals = Arc::new(crate::approvals::Approvals::default());
    let extension = NoteExtension::default().with_note("a", "alpha");
    let fixture = setup_with(extension.clone(), executing_specs(), approvals.clone());
    let failure = answer(&fixture, &approvals, ApprovalDecision::Deny)
        .await
        .unwrap_err();
    assert_eq!(failure.code, CapabilityFailureCode::APPROVAL_DENIED);
    assert_eq!(extension.executions(), 0);
    let records = fixture.store.tool_invocations();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].failure.as_ref().unwrap().code,
        CapabilityFailureCode::APPROVAL_DENIED
    );
    let events = fixture.output.0.lock().unwrap();
    assert!(matches!(
        events.as_slice(),
        [
            AgentEvent::ApprovalRequested { tool, .. },
            AgentEvent::ApprovalResolved { decision: ApprovalDecision::Deny, .. },
            AgentEvent::ToolInvocationStarted { .. },
            AgentEvent::ToolInvocationFailed { .. },
        ] if tool == READ_NOTE
    ));
}

#[tokio::test]
async fn project_approval_is_remembered_for_later_calls() {
    let approvals = Arc::new(crate::approvals::Approvals::default());
    let extension = NoteExtension::default().with_note("a", "alpha");
    let fixture = setup_with(extension.clone(), executing_specs(), approvals.clone());
    answer(&fixture, &approvals, ApprovalDecision::AllowForProject)
        .await
        .unwrap();
    fixture
        .executor
        .execute(call(READ_NOTE, json!({"id": "a"})), fixture.executor.now())
        .await
        .unwrap();
    assert_eq!(extension.executions(), 2);
    let requests = fixture
        .output
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|event| matches!(event, AgentEvent::ApprovalRequested { .. }))
        .count();
    assert_eq!(requests, 1);
}

#[tokio::test]
async fn cancelling_the_turn_answers_a_pending_approval() {
    let approvals = Arc::new(crate::approvals::Approvals::default());
    let extension = NoteExtension::default().with_note("a", "alpha");
    let fixture = setup_with(extension.clone(), executing_specs(), approvals);
    let execution = fixture
        .executor
        .execute(call(READ_NOTE, json!({"id": "a"})), fixture.executor.now());
    tokio::pin!(execution);
    while !fixture
        .output
        .0
        .lock()
        .unwrap()
        .iter()
        .any(|event| matches!(event, AgentEvent::ApprovalRequested { .. }))
    {
        tokio::select! {
            _ = tokio::task::yield_now() => {}
            _ = &mut execution => panic!("the call must wait for the user"),
        }
    }
    fixture.cancellation.cancel(CancellationReason::User);
    let failure = execution.await.unwrap_err();
    assert_eq!(failure.code, CapabilityFailureCode::CANCELLED);
    assert_eq!(extension.executions(), 0);
}
