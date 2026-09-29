use crate::{CompiledWorkflow, HarnessError, HarnessHost, HarnessPorts};
use yss_harness_contract::*;

use super::*;
use crate::test_support::{FixedClock, InMemoryHarnessStore, MockAgentDriver, SequentialIds};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use yss_harness_contract::{
    CapabilityControl, CapabilityFuture, CapabilityGatewayPort, CapabilityInvocationContext,
    DatasetSchemaInspection, InspectDatasetSchemaRequest, UnixMillis, WorkflowDefinition,
    WorkflowId, WorkflowStep, WorkflowStepId, WorkflowStepState, WorkflowVersion,
};
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

#[derive(Default)]
struct BlockingGateway {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    calls: AtomicUsize,
    control: Mutex<Option<CapabilityControl>>,
}

impl CapabilityGatewayPort for BlockingGateway {
    fn invoke<'a>(
        &'a self,
        _: CapabilityInvocationContext,
        _: AutomationCapabilityRequest,
        control: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            *self.control.lock().unwrap() = Some(control);
            self.entered.notify_one();
            // Deliberately return success even after cancellation. The Host must
            // fence both a late success and the executor's cancellation outcome.
            self.release.notified().await;
            Ok(AutomationCapabilityResult::DatasetSchemaInspection(
                DatasetSchemaInspection {
                    database_id: "database-1".into(),
                    runtime_revision: 1,
                    schema_revision: 1,
                    columns: vec![],
                },
            ))
        })
    }
}

async fn planned() -> (
    Arc<HarnessHost>,
    Arc<InMemoryHarnessStore>,
    Arc<BlockingGateway>,
    WorkflowRunRecord,
) {
    let store = Arc::new(InMemoryHarnessStore::default());
    let gateway = Arc::new(BlockingGateway::default());
    let host = Arc::new(
        HarnessHost::new(HarnessPorts {
            agent_driver: Arc::new(MockAgentDriver::new("Done")),
            capability_gateway: gateway.clone(),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: store.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            memory: store.clone(),
            approvals: store.clone(),
            clock: Arc::new(FixedClock::new(1000)),
            ids: Arc::new(SequentialIds::default()),
        })
        .unwrap(),
    );
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
        id: HarnessTurnId::try_new("workflow-turn").unwrap(),
        session_id: session.id.clone(),
        state: HarnessTurnState::Completed,
        user_message: "Inspect schema".into(),
        final_text: None,
        started_at: UnixMillis::from_existing(1000),
        finished_at: Some(UnixMillis::from_existing(1000)),
    };
    store.create_turn(&turn).await.unwrap();
    let compiled = CompiledWorkflow::compile(WorkflowDefinition {
        id: WorkflowId::try_new("read_schema").unwrap(),
        version: WorkflowVersion::try_new("1.0.0").unwrap(),
        steps: vec![WorkflowStep {
            id: WorkflowStepId::try_new("inspect").unwrap(),
            depends_on: vec![],
            request: AutomationCapabilityRequest::InspectDatasetSchema(
                InspectDatasetSchemaRequest {
                    database_id: "database-1".into(),
                },
            ),
        }],
    })
    .unwrap();
    let run = host
        .plan_workflow(&session.id, Some(&turn.id), &compiled)
        .await
        .unwrap();
    (host, store, gateway, run)
}

#[tokio::test]
async fn recovery_terminates_an_interrupted_write_even_when_the_workflow_was_paused() {
    let (host, store, gateway, read_run) = planned().await;
    let step_id = WorkflowStepId::try_new("save").unwrap();
    let compiled = CompiledWorkflow::compile(WorkflowDefinition {
        id: WorkflowId::try_new("save_graph").unwrap(),
        version: WorkflowVersion::try_new("1.0.0").unwrap(),
        steps: vec![WorkflowStep {
            id: step_id.clone(),
            depends_on: vec![],
            request: AutomationCapabilityRequest::SaveGraph(SaveGraphRequest {
                graph_path: "events/Main.yssbi-event".into(),
                graph_hash: "0".repeat(64),
            }),
        }],
    })
    .unwrap();
    let mut run = host
        .plan_workflow(&read_run.session_id, read_run.turn_id.as_ref(), &compiled)
        .await
        .unwrap();
    let now = UnixMillis::from_existing(1001);
    crate::WorkflowRuntime::start(&mut run, now).unwrap();
    crate::WorkflowRuntime::start_step(&compiled, &mut run, &step_id, now).unwrap();
    crate::WorkflowRuntime::pause(&mut run, now).unwrap();
    store.save_run(&run, Some(run.revision)).await.unwrap();

    assert_eq!(host.recover_workflows().await.unwrap(), 1);
    let recovered = store.load_run(&run.id).await.unwrap().unwrap();
    assert_eq!(recovered.state, WorkflowRunState::Failed);
    assert_eq!(
        recovered.steps[&step_id].state,
        WorkflowStepState::TerminalFailure
    );
    assert_eq!(recovered.steps[&step_id].attempt, 1);
    assert_eq!(host.advance_workflow(&run.id).await.unwrap(), recovered);
    assert!(host.resume_workflow(&run.id).await.is_err());
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn workflow_cancel_fences_late_results_and_concurrent_advance() {
    let (host, store, gateway, run) = planned().await;
    let advance = {
        let host = host.clone();
        let id = run.id.clone();
        tokio::spawn(async move { host.advance_workflow(&id).await })
    };
    tokio::time::timeout(Duration::from_secs(5), gateway.entered.notified())
        .await
        .unwrap();
    assert!(matches!(
        host.advance_workflow(&run.id).await,
        Err(HarnessError::ConcurrentWorkflow)
    ));
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 1);
    let cancelled = tokio::time::timeout(Duration::from_secs(5), host.cancel_workflow(&run.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cancelled.state, WorkflowRunState::Cancelled);
    assert_eq!(
        gateway
            .control
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .check()
            .unwrap_err()
            .code,
        CapabilityFailureCode::Cancelled
    );
    gateway.release.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(5), advance)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(result, cancelled);
    assert_eq!(store.load_run(&run.id).await.unwrap().unwrap(), cancelled);
    assert!(!store.published_events().iter().any(|event| matches!(
        event.event,
        HarnessEvent::WorkflowCompleted { .. } | HarnessEvent::WorkflowStepCompleted { .. }
    )));
}

#[tokio::test]
async fn workflow_pause_and_resume_preserve_new_state_while_the_step_settles() {
    for resume_before_completion in [false, true] {
        let (host, store, gateway, run) = planned().await;
        let advance = {
            let host = host.clone();
            let id = run.id.clone();
            tokio::spawn(async move { host.advance_workflow(&id).await })
        };
        tokio::time::timeout(Duration::from_secs(5), gateway.entered.notified())
            .await
            .unwrap();
        let paused = host.pause_workflow(&run.id).await.unwrap();
        assert_eq!(paused.state, WorkflowRunState::Paused);
        if resume_before_completion {
            host.resume_workflow(&run.id).await.unwrap();
        }
        assert!(matches!(
            host.advance_workflow(&run.id).await,
            Err(HarnessError::ConcurrentWorkflow)
        ));
        gateway.release.notify_one();
        let settled = tokio::time::timeout(Duration::from_secs(5), advance)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(
            settled.state,
            if resume_before_completion {
                WorkflowRunState::Ready
            } else {
                WorkflowRunState::Paused
            }
        );
        assert_eq!(
            settled.steps.values().next().unwrap().state,
            WorkflowStepState::Succeeded
        );
        assert_eq!(store.load_run(&run.id).await.unwrap().unwrap(), settled);
        if !resume_before_completion {
            assert!(matches!(
                host.advance_workflow(&run.id).await,
                Err(HarnessError::WorkflowWaiting)
            ));
            host.resume_workflow(&run.id).await.unwrap();
        }
        assert_eq!(
            host.advance_workflow(&run.id).await.unwrap().state,
            WorkflowRunState::Completed
        );
        assert_eq!(gateway.calls.load(Ordering::SeqCst), 1);
    }
}
