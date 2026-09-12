use std::collections::BTreeMap;
use std::future::{Future, poll_fn};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use yss_automation_contract::{
    AgentEvent, AgentEventOutput, ApprovalGrantId, AutomationIdKind, AutomationIdentityError,
    CancellationToken, CapabilityControl, CapabilityFailure, CapabilityFailureCode,
    CapabilityGatewayPort, CapabilityId, CapabilityInvocationContext, CapabilityInvocationId,
    ClockPort, HarnessSessionId, HarnessTurnId, IdGeneratorPort, IdempotencyKey,
    ModelCapabilityExecutor, ModelCapabilityOutcome, ModelCapabilityRequest, PrincipalId,
    ProjectSessionBinding, ToolDescriptor, ToolEffect, ToolInvocationBegin, ToolInvocationId,
    ToolInvocationLedgerPort, ToolInvocationRecord, ToolInvocationState, WorkflowRunId,
    WorkflowStepId,
};

#[derive(Clone, Debug)]
pub struct ToolRegistry {
    descriptors: BTreeMap<CapabilityId, ToolDescriptor>,
}

impl ToolRegistry {
    pub fn graph_assistant() -> Result<Self, AutomationIdentityError> {
        let mut registry = Self::read_only_foundation()?;
        for capability in [
            CapabilityId::ApplyGraphEdit,
            CapabilityId::CompileGraph,
            CapabilityId::ExecuteGraph,
            CapabilityId::SaveGraph,
            CapabilityId::ListGraphResults,
        ] {
            registry
                .descriptors
                .insert(capability, ToolDescriptor::for_capability(capability)?);
        }
        Ok(registry)
    }
    pub fn read_only_foundation() -> Result<Self, AutomationIdentityError> {
        let descriptors = [
            CapabilityId::InspectGraph,
            CapabilityId::SearchNodeCatalog,
            CapabilityId::InspectDatasetSchema,
            CapabilityId::InspectDatasetProfile,
            CapabilityId::InspectResult,
            CapabilityId::InspectProject,
        ]
        .into_iter()
        .map(|capability_id| {
            ToolDescriptor::for_capability(capability_id)
                .map(|descriptor| (capability_id, descriptor))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(Self { descriptors })
    }

    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        self.descriptors.values().cloned().collect()
    }

    pub fn descriptor(&self, capability_id: CapabilityId) -> Option<&ToolDescriptor> {
        self.descriptors.get(&capability_id)
    }

    pub(crate) fn with_approved_capability(
        mut self,
        capability_id: CapabilityId,
    ) -> Result<Self, AutomationIdentityError> {
        self.descriptors.insert(
            capability_id,
            ToolDescriptor::for_capability(capability_id)?,
        );
        Ok(self)
    }
}

pub(crate) struct HarnessToolExecutor {
    registry: ToolRegistry,
    gateway: Arc<dyn CapabilityGatewayPort>,
    ledger: Arc<dyn ToolInvocationLedgerPort>,
    clock: Arc<dyn ClockPort>,
    ids: Arc<dyn IdGeneratorPort>,
    principal_id: PrincipalId,
    session_id: HarnessSessionId,
    turn_id: HarnessTurnId,
    project: ProjectSessionBinding,
    cancellation: CancellationToken,
    workflow_run_id: Option<WorkflowRunId>,
    workflow_step_id: Option<WorkflowStepId>,
    approval_grant_id: Option<ApprovalGrantId>,
    output: Option<Arc<dyn AgentEventOutput>>,
}

impl HarnessToolExecutor {
    #[allow(
        clippy::too_many_arguments,
        reason = "the executor captures one explicit invocation authority envelope"
    )]
    pub(crate) fn new(
        registry: ToolRegistry,
        gateway: Arc<dyn CapabilityGatewayPort>,
        ledger: Arc<dyn ToolInvocationLedgerPort>,
        clock: Arc<dyn ClockPort>,
        ids: Arc<dyn IdGeneratorPort>,
        principal_id: PrincipalId,
        session_id: HarnessSessionId,
        turn_id: HarnessTurnId,
        project: ProjectSessionBinding,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            registry,
            gateway,
            ledger,
            clock,
            ids,
            principal_id,
            session_id,
            turn_id,
            project,
            cancellation,
            workflow_run_id: None,
            workflow_step_id: None,
            approval_grant_id: None,
            output: None,
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "workflow execution captures explicit authority plus durable step identity"
    )]
    pub(crate) fn new_for_workflow(
        registry: ToolRegistry,
        gateway: Arc<dyn CapabilityGatewayPort>,
        ledger: Arc<dyn ToolInvocationLedgerPort>,
        clock: Arc<dyn ClockPort>,
        ids: Arc<dyn IdGeneratorPort>,
        principal_id: PrincipalId,
        session_id: HarnessSessionId,
        turn_id: HarnessTurnId,
        project: ProjectSessionBinding,
        cancellation: CancellationToken,
        workflow_run_id: WorkflowRunId,
        workflow_step_id: WorkflowStepId,
    ) -> Self {
        Self {
            registry,
            gateway,
            ledger,
            clock,
            ids,
            principal_id,
            session_id,
            turn_id,
            project,
            cancellation,
            workflow_run_id: Some(workflow_run_id),
            workflow_step_id: Some(workflow_step_id),
            approval_grant_id: None,
            output: None,
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "approved execution captures one exact principal/session/turn/request authority envelope"
    )]
    pub(crate) fn new_approved(
        registry: ToolRegistry,
        gateway: Arc<dyn CapabilityGatewayPort>,
        ledger: Arc<dyn ToolInvocationLedgerPort>,
        clock: Arc<dyn ClockPort>,
        ids: Arc<dyn IdGeneratorPort>,
        principal_id: PrincipalId,
        session_id: HarnessSessionId,
        turn_id: HarnessTurnId,
        project: ProjectSessionBinding,
        approval_grant_id: ApprovalGrantId,
    ) -> Self {
        Self {
            registry,
            gateway,
            ledger,
            clock,
            ids,
            principal_id,
            session_id,
            turn_id,
            project,
            cancellation: CancellationToken::default(),
            workflow_run_id: None,
            workflow_step_id: None,
            approval_grant_id: Some(approval_grant_id),
            output: None,
        }
    }

    pub(crate) fn with_output(mut self, output: Arc<dyn AgentEventOutput>) -> Self {
        self.output = Some(output);
        self
    }

    async fn emit(&self, event: AgentEvent) -> Result<(), CapabilityFailure> {
        if let Some(output) = &self.output {
            output
                .emit(event)
                .await
                .map_err(|_| persistence_unavailable())?;
        }
        Ok(())
    }

    async fn execute_request(
        &self,
        request: ModelCapabilityRequest,
    ) -> Result<ModelCapabilityOutcome, CapabilityFailure> {
        if self.cancellation.is_cancelled() {
            return Err(CapabilityFailure::new(CapabilityFailureCode::Cancelled));
        }
        let capability_id = request.request.capability_id();
        let descriptor = self.registry.descriptor(capability_id).ok_or_else(|| {
            CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
                .with_detail("capabilityId", capability_id.as_str())
        })?;
        request.request.validate().map_err(|_| {
            CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
                .with_detail("capabilityId", capability_id.as_str())
        })?;

        let raw_invocation_id = self
            .ids
            .next_id(AutomationIdKind::ToolInvocation)
            .map_err(|_| persistence_unavailable())?;
        let invocation_id = ToolInvocationId::try_new(raw_invocation_id.clone())
            .map_err(|_| persistence_unavailable())?;
        let idempotency_key =
            if let yss_automation_contract::AutomationCapabilityRequest::ApplyGraphEdit(edit) =
                &request.request
            {
                let digest = yss_canonical_hash::hash_canonical(
                    "yssbi.assistant.graph-edit.idempotency.v1",
                    &(&self.session_id, &self.turn_id, &edit.client_key),
                )
                .map_err(|_| persistence_unavailable())?;
                IdempotencyKey::try_new(
                    digest
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>(),
                )
                .map_err(|_| persistence_unavailable())?
            } else {
                IdempotencyKey::try_new(raw_invocation_id).map_err(|_| persistence_unavailable())?
            };
        let capability_invocation_id = CapabilityInvocationId::try_new(
            self.ids
                .next_id(AutomationIdKind::CapabilityInvocation)
                .map_err(|_| persistence_unavailable())?,
        )
        .map_err(|_| persistence_unavailable())?;
        let started_at = self.clock.now();
        let deadline = started_at
            .checked_add(descriptor.timeout_ms)
            .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::DeadlineElapsed))?;
        let mut record = ToolInvocationRecord {
            id: invocation_id.clone(),
            idempotency_key,
            session_id: self.session_id.clone(),
            turn_id: self.turn_id.clone(),
            workflow_run_id: self.workflow_run_id.clone(),
            workflow_step_id: self.workflow_step_id.clone(),
            project: self.project.clone(),
            capability_id,
            request: request.request.clone(),
            state: ToolInvocationState::Running,
            result: None,
            failure: None,
            started_at,
            deadline,
            finished_at: None,
        };
        match self
            .ledger
            .begin(&record)
            .await
            .map_err(|_| persistence_unavailable())?
        {
            ToolInvocationBegin::Started => {}
            ToolInvocationBegin::Existing(existing) => {
                if existing.request != request.request {
                    return Err(CapabilityFailure::new(
                        CapabilityFailureCode::InvocationConflict,
                    ));
                }
                return replay_existing(*existing);
            }
        }

        let context = CapabilityInvocationContext::new(
            self.principal_id.clone(),
            self.session_id.clone(),
            capability_invocation_id,
            self.project.clone(),
        );
        let context = match &self.approval_grant_id {
            Some(grant_id) => context.with_approval(grant_id.clone()),
            None => context,
        };
        let control = CapabilityControl::new(
            self.cancellation.clone(),
            Duration::from_millis(deadline.get().saturating_sub(self.clock.now().get())),
        );
        let started = self
            .emit(AgentEvent::ToolInvocationStarted {
                invocation_id: invocation_id.clone(),
                capability_id,
            })
            .await;
        let mut outcome = match started {
            Err(error) => Err(error),
            Ok(()) => {
                // Catch adapter panics while polling so the authoritative ledger still closes.
                let mut invocation = Box::pin(async {
                    self.gateway
                        .invoke(context, request.request, control.clone())
                        .await
                });
                poll_fn(|context| {
                    match catch_unwind(AssertUnwindSafe(|| invocation.as_mut().poll(context))) {
                        Ok(result) => result,
                        Err(_) => Poll::Ready(Err(CapabilityFailure::new(
                            CapabilityFailureCode::InternalFailure,
                        ))),
                    }
                })
                .await
            }
        };
        if outcome
            .as_ref()
            .is_ok_and(|result| result.capability_id() != capability_id)
        {
            outcome = Err(CapabilityFailure::new(
                CapabilityFailureCode::InternalFailure,
            ));
        }
        if outcome.as_ref().is_ok_and(|result| {
            result
                .validate_budget(descriptor.result_budget.maximum_bytes as usize)
                .is_err()
        }) {
            outcome = Err(CapabilityFailure::new(
                CapabilityFailureCode::ResultTooLarge,
            ));
        }
        if !outcome
            .as_ref()
            .is_err_and(|error| error.code == CapabilityFailureCode::OutcomeUnknown)
            && (descriptor.effect == ToolEffect::Inspect || outcome.is_err())
        {
            if let Err(failure) = control.check() {
                outcome = Err(failure);
            } else if self.clock.now() >= deadline {
                outcome = Err(CapabilityFailure::new(
                    CapabilityFailureCode::DeadlineElapsed,
                ));
            }
        }
        record.finished_at = Some(self.clock.now());
        match &outcome {
            Ok(result) => {
                record.state = ToolInvocationState::Succeeded;
                record.result = Some(result.clone());
            }
            Err(failure) => {
                record.state = ToolInvocationState::Failed;
                record.failure = Some(failure.clone());
            }
        }
        self.ledger
            .finish(&record)
            .await
            .map_err(|_| persistence_unavailable())?;

        self.emit(match &outcome {
            Ok(_) => AgentEvent::ToolInvocationCompleted {
                invocation_id: invocation_id.clone(),
                capability_id,
            },
            Err(failure) => AgentEvent::ToolInvocationFailed {
                invocation_id: invocation_id.clone(),
                capability_id,
                failure_code: failure.code,
            },
        })
        .await?;

        outcome.map(|result| ModelCapabilityOutcome {
            invocation_id,
            result,
        })
    }
}

impl ModelCapabilityExecutor for HarnessToolExecutor {
    fn execute<'a>(
        &'a self,
        request: ModelCapabilityRequest,
    ) -> yss_automation_contract::AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>>
    {
        Box::pin(async move { self.execute_request(request).await })
    }
}

fn replay_existing(
    existing: ToolInvocationRecord,
) -> Result<ModelCapabilityOutcome, CapabilityFailure> {
    if let Some(result) = existing.result {
        return Ok(ModelCapabilityOutcome {
            invocation_id: existing.id,
            result,
        });
    }
    if let Some(failure) = existing.failure {
        return Err(failure);
    }
    Err(CapabilityFailure::new(
        CapabilityFailureCode::InvocationConflict,
    ))
}

fn persistence_unavailable() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::PersistenceUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FixedClock, InMemoryHarnessStore, SequentialIds};
    use std::sync::Mutex;
    use yss_automation_contract::{
        AgentFuture, AgentOutputFailure, AutomationCapabilityRequest, CancellationReason,
        CapabilityFuture, InspectDatasetProfileRequest,
    };
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    struct FailingGateway(u8, Arc<FixedClock>);
    impl CapabilityGatewayPort for FailingGateway {
        fn invoke<'a>(
            &'a self,
            _: CapabilityInvocationContext,
            _: AutomationCapabilityRequest,
            control: CapabilityControl,
        ) -> CapabilityFuture<'a> {
            Box::pin(async move {
                match self.0 {
                    1 => panic!("synthetic adapter panic"),
                    2 => {
                        control.cancellation().cancel(CancellationReason::User);
                    }
                    3 => {
                        self.1.advance(31_000);
                    }
                    _ => {}
                }
                Err(CapabilityFailure::new(
                    CapabilityFailureCode::DatabaseUnavailable,
                ))
            })
        }
    }
    #[derive(Default)]
    struct Output(Mutex<Vec<AgentEvent>>);
    impl AgentEventOutput for Output {
        fn emit<'a>(
            &'a self,
            event: AgentEvent,
        ) -> AgentFuture<'a, Result<(), AgentOutputFailure>> {
            Box::pin(async move {
                self.0.lock().unwrap().push(event);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn tool_failures_close_the_ledger_and_emit_the_identified_terminal_event() {
        for (mode, expected) in [
            (0, CapabilityFailureCode::DatabaseUnavailable),
            (1, CapabilityFailureCode::InternalFailure),
            (2, CapabilityFailureCode::Cancelled),
            (3, CapabilityFailureCode::DeadlineElapsed),
        ] {
            let store = Arc::new(InMemoryHarnessStore::default());
            let clock = Arc::new(FixedClock::new(1000));
            let output = Arc::new(Output::default());
            let executor = HarnessToolExecutor::new(
                ToolRegistry::read_only_foundation().unwrap(),
                Arc::new(FailingGateway(mode, clock.clone())),
                store.clone(),
                clock,
                Arc::new(SequentialIds::default()),
                PrincipalId::try_new("user-1").unwrap(),
                HarnessSessionId::try_new("session-1").unwrap(),
                HarnessTurnId::try_new("turn-1").unwrap(),
                ProjectSessionBinding::new(
                    ProjectInstanceId::from_existing("project-1".into()),
                    ProjectSessionId::new("project-session-1"),
                ),
                CancellationToken::default(),
            )
            .with_output(output.clone());
            let result = executor
                .execute(ModelCapabilityRequest {
                    request: AutomationCapabilityRequest::InspectDatasetProfile(
                        InspectDatasetProfileRequest {
                            database_id: "data-1".into(),
                        },
                    ),
                })
                .await;
            assert_eq!(result.unwrap_err().code, expected);
            let records = store.tool_invocations();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].state, ToolInvocationState::Failed);
            assert_eq!(records[0].failure.as_ref().unwrap().code, expected);
            assert!(records[0].finished_at.is_some());
            assert!(store.load_running_invocations().await.unwrap().is_empty());
            assert!(matches!(output.0.lock().unwrap().as_slice(), [
                AgentEvent::ToolInvocationStarted { invocation_id: started, .. },
                AgentEvent::ToolInvocationFailed { invocation_id: finished, failure_code, .. },
            ] if started == finished && started == &records[0].id && *failure_code == expected));
        }
    }

    #[test]
    fn default_registry_keeps_mutating_capabilities_unrouted() {
        let registry = ToolRegistry::read_only_foundation().unwrap();

        assert!(registry.descriptor(CapabilityId::InspectGraph).is_some());
        assert!(registry.descriptor(CapabilityId::ApplyGraphEdit).is_none());
        let editor = ToolRegistry::graph_assistant().unwrap();
        for id in [
            CapabilityId::ApplyGraphEdit,
            CapabilityId::CompileGraph,
            CapabilityId::ExecuteGraph,
            CapabilityId::SaveGraph,
            CapabilityId::ListGraphResults,
        ] {
            assert!(editor.descriptor(id).is_some());
        }
    }

    #[tokio::test]
    async fn graph_edit_retries_reuse_the_receipt_and_reject_changed_requests_with_the_same_key() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use yss_automation_contract::{
            ApplyGraphEditRequest, AutomationCapabilityResult, GraphEditOperation,
            GraphEditPosition, GraphEditReceipt,
        };
        struct Gateway(AtomicUsize);
        impl CapabilityGatewayPort for Gateway {
            fn invoke<'a>(
                &'a self,
                _: CapabilityInvocationContext,
                _: AutomationCapabilityRequest,
                _: CapabilityControl,
            ) -> CapabilityFuture<'a> {
                Box::pin(async {
                    self.0.fetch_add(1, Ordering::SeqCst);
                    Ok(AutomationCapabilityResult::GraphEditReceipt(
                        GraphEditReceipt {
                            graph_path: "events/Main.yssbi-event".into(),
                            from_revision: 0,
                            to_revision: 1,
                            client_key: "batch-1".into(),
                            graph_hash: "1".repeat(64),
                            created_nodes: BTreeMap::new(),
                            created_ports: BTreeMap::new(),
                        },
                    ))
                })
            }
        }
        let gateway = Arc::new(Gateway(AtomicUsize::new(0)));
        let store = Arc::new(InMemoryHarnessStore::default());
        let executor = HarnessToolExecutor::new(
            ToolRegistry::graph_assistant().unwrap(),
            gateway.clone(),
            store.clone(),
            Arc::new(FixedClock::new(1000)),
            Arc::new(SequentialIds::default()),
            PrincipalId::try_new("user-1").unwrap(),
            HarnessSessionId::try_new("session-1").unwrap(),
            HarnessTurnId::try_new("turn-1").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-1"),
            ),
            CancellationToken::default(),
        );
        let mut edit = ApplyGraphEditRequest {
            graph_path: "events/Main.yssbi-event".into(),
            graph_hash: "0".repeat(64),
            base_revision: 0,
            client_key: "batch-1".into(),
            locale: "en-US".into(),
            operations: vec![GraphEditOperation::MoveNodes {
                positions: vec![GraphEditPosition {
                    node_id: "node-1".into(),
                    x: 1.,
                    y: 2.,
                }],
            }],
        };
        let first = executor
            .execute(ModelCapabilityRequest {
                request: AutomationCapabilityRequest::ApplyGraphEdit(edit.clone()),
            })
            .await
            .unwrap();
        let retry = executor
            .execute(ModelCapabilityRequest {
                request: AutomationCapabilityRequest::ApplyGraphEdit(edit.clone()),
            })
            .await
            .unwrap();
        assert_eq!(first, retry);
        edit.base_revision = 1;
        let conflict = executor
            .execute(ModelCapabilityRequest {
                request: AutomationCapabilityRequest::ApplyGraphEdit(edit),
            })
            .await
            .unwrap_err();
        assert_eq!(conflict.code, CapabilityFailureCode::InvocationConflict);
        assert_eq!(gateway.0.load(Ordering::SeqCst), 1);
        assert_eq!(store.tool_invocations().len(), 1);
    }
}
