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
    let store = Arc::new(InMemoryHarnessStore::default());
    let clock = Arc::new(FixedClock::new(1000));
    let output = Arc::new(Output::default());
    let cancellation = CancellationToken::default();
    let executor = HarnessToolExecutor::new(
        ToolRegistry::new(note_specs(true)),
        ToolServices {
            extension: Arc::new(extension),
            knowledge: Arc::new(crate::KnowledgeService::new(
                store.clone(),
                Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
            )),
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
        assert_eq!(record.finished_at.unwrap().get() - record.started_at.get(), 37);
    }

    let fixture = setup(NoteExtension::default());
    let failure = fixture
        .executor
        .execute(call("unregistered", json!({"id": "a"})), fixture.executor.now())
        .await
        .unwrap_err();
    assert_eq!(failure.code, CapabilityFailureCode::INVALID_REQUEST);
    assert_eq!(failure.details["capabilityId"], "unregistered");
}
