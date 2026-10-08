use std::future::{Future, poll_fn};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use yss_harness_contract::{
    AgentEvent, AgentEventOutput, ApprovalGrantId, AutomationCapabilityRequest, AutomationIdKind,
    CancellationToken, CapabilityControl, CapabilityFailure, CapabilityFailureCode,
    CapabilityGatewayPort, CapabilityId, CapabilityInvocationContext, CapabilityInvocationId,
    ClockPort, HarnessSessionId, HarnessTurnId, IdGeneratorPort, IdempotencyKey,
    ModelCapabilityOutcome, PrincipalId, ProjectSessionBinding, ToolDescriptor, ToolEffect,
    ToolInvocationBegin, ToolInvocationId, ToolInvocationLedgerPort, ToolInvocationRecord,
    ToolInvocationRequest, ToolInvocationState, UnixMillis, WorkflowRunId, WorkflowStepId,
};

#[derive(Clone, Debug)]
pub struct ToolRegistry {
    capabilities: std::collections::BTreeSet<CapabilityId>,
}

impl ToolRegistry {
    pub fn for_agent(role: yss_harness_contract::AgentRole) -> Self {
        Self {
            capabilities: crate::agent_definition(role)
                .capabilities
                .iter()
                .copied()
                .collect(),
        }
    }

    pub(crate) fn for_capability(capability_id: CapabilityId) -> Self {
        Self {
            capabilities: [capability_id].into_iter().collect(),
        }
    }

    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        self.capabilities
            .iter()
            .copied()
            .map(ToolDescriptor::for_capability)
            .collect()
    }

    pub(crate) fn with_mode(mut self, mode: yss_harness_contract::HarnessMode) -> Self {
        self.capabilities
            .retain(|capability| mode.allows(*capability));
        self
    }

    pub fn descriptor(
        &self,
        capability_id: CapabilityId,
    ) -> Option<&'static yss_harness_contract::CapabilityDescriptor> {
        self.capabilities
            .contains(&capability_id)
            .then(|| capability_id.descriptor())
    }
}

pub(crate) struct HarnessToolExecutor {
    registry: ToolRegistry,
    gateway: Arc<dyn CapabilityGatewayPort>,
    knowledge: Arc<crate::KnowledgeService>,
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
    agent: Option<Arc<std::sync::Mutex<yss_harness_contract::AgentInvocationScope>>>,
}

impl HarnessToolExecutor {
    pub(crate) async fn begin_control(
        &self,
        tool: yss_harness_contract::AgentControlTool,
    ) -> Result<ToolInvocationId, CapabilityFailure> {
        let id = self
            .ids
            .next_id(AutomationIdKind::ToolInvocation)
            .map_err(|_| persistence_unavailable())?;
        let invocation_id = ToolInvocationId::try_new(id).map_err(|_| persistence_unavailable())?;
        self.emit(AgentEvent::ControlToolStarted {
            invocation_id: invocation_id.clone(),
            tool,
        })
        .await?;
        if self.cancellation.is_cancelled() {
            self.finish_control(
                invocation_id,
                tool,
                Some(CapabilityFailure::new(CapabilityFailureCode::Cancelled)),
            )
            .await?;
            return Err(CapabilityFailure::new(CapabilityFailureCode::Cancelled));
        }
        Ok(invocation_id)
    }

    pub(crate) async fn finish_control(
        &self,
        invocation_id: ToolInvocationId,
        tool: yss_harness_contract::AgentControlTool,
        failure: Option<CapabilityFailure>,
    ) -> Result<(), CapabilityFailure> {
        self.emit(AgentEvent::ControlToolFinished {
            invocation_id,
            tool,
            failure_code: failure.as_ref().map(|failure| failure.code),
            failure_details: failure.map(|failure| failure.details),
        })
        .await
    }

    pub(crate) fn new_call_key(&self) -> Result<String, CapabilityFailure> {
        let id = self
            .ids
            .next_id(AutomationIdKind::ToolInvocation)
            .map_err(|_| persistence_unavailable())?;
        let digest = yss_canonical_hash::hash_canonical("yssbi.harness.call", &id)
            .map_err(|_| persistence_unavailable())?;
        Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the executor captures one explicit invocation authority envelope"
    )]
    pub(crate) fn new(
        registry: ToolRegistry,
        gateway: Arc<dyn CapabilityGatewayPort>,
        knowledge: Arc<crate::KnowledgeService>,
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
            knowledge,
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
            agent: None,
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "workflow execution captures explicit authority plus durable step identity"
    )]
    pub(crate) fn new_for_workflow(
        registry: ToolRegistry,
        gateway: Arc<dyn CapabilityGatewayPort>,
        knowledge: Arc<crate::KnowledgeService>,
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
            knowledge,
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
            agent: None,
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "approved execution captures one exact principal/session/turn/request authority envelope"
    )]
    pub(crate) fn new_approved(
        registry: ToolRegistry,
        gateway: Arc<dyn CapabilityGatewayPort>,
        knowledge: Arc<crate::KnowledgeService>,
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
            knowledge,
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
            agent: None,
        }
    }

    pub(crate) fn with_output(mut self, output: Arc<dyn AgentEventOutput>) -> Self {
        self.output = Some(output);
        self
    }

    pub(crate) fn with_agent(
        mut self,
        agent: Arc<std::sync::Mutex<yss_harness_contract::AgentInvocationScope>>,
    ) -> Self {
        self.agent = Some(agent);
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

    pub(crate) async fn execute(
        &self,
        request: AutomationCapabilityRequest,
        graph_observation: Option<String>,
    ) -> Result<ModelCapabilityOutcome, CapabilityFailure> {
        self.execute_started(request, graph_observation, self.now())
            .await
    }

    pub(crate) fn now(&self) -> UnixMillis {
        self.clock.now()
    }

    pub(crate) async fn execute_started(
        &self,
        request: AutomationCapabilityRequest,
        graph_observation: Option<String>,
        started_at: UnixMillis,
    ) -> Result<ModelCapabilityOutcome, CapabilityFailure> {
        self.execute_invocation(request.into(), graph_observation, started_at, None)
            .await
    }

    pub(crate) async fn reject(
        &self,
        capability_id: CapabilityId,
        input: Option<yss_harness_contract::model::CapabilityInput>,
        failure: CapabilityFailure,
        started_at: UnixMillis,
    ) -> Result<ModelCapabilityOutcome, CapabilityFailure> {
        self.execute_invocation(
            ToolInvocationRequest::Rejected {
                capability_id,
                input,
            },
            None,
            started_at,
            Some(failure),
        )
        .await
    }

    async fn execute_invocation(
        &self,
        request: ToolInvocationRequest,
        graph_observation: Option<String>,
        started_at: UnixMillis,
        rejection: Option<CapabilityFailure>,
    ) -> Result<ModelCapabilityOutcome, CapabilityFailure> {
        let agent = self
            .agent
            .as_ref()
            .map(|scope| scope.lock().unwrap_or_else(|e| e.into_inner()).clone());
        let capability_id = request.capability_id();
        let descriptor = capability_id.descriptor();
        let validation = (|| {
            if let Some(failure) = rejection {
                return Err(failure);
            }
            let request = request
                .bound()
                .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::InternalFailure))?;
            self.registry.descriptor(capability_id).ok_or_else(|| {
                CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
                    .with_detail("capabilityId", capability_id.as_str())
            })?;
            if let Some(scope) = &agent {
                crate::authorize_agent_capability(scope, request)?;
            }
            request
                .validate()
                .map_err(|error| error.into_failure(capability_id))
        })();

        let raw_invocation_id = self
            .ids
            .next_id(AutomationIdKind::ToolInvocation)
            .map_err(|_| persistence_unavailable())?;
        let invocation_id = ToolInvocationId::try_new(raw_invocation_id.clone())
            .map_err(|_| persistence_unavailable())?;
        let client_key = request.bound().and_then(|request| match request {
            AutomationCapabilityRequest::ApplyGraphEdit(edit) => Some(edit.client_key.as_str()),
            AutomationCapabilityRequest::GraphMutation(edit) => Some(edit.client_key.as_str()),
            _ => None,
        });
        let idempotency_key = if let Some(client_key) = client_key {
            let digest = yss_canonical_hash::hash_canonical(
                "yssbi.assistant.graph-edit.idempotency.v1",
                &(
                    &self.session_id,
                    &self.turn_id,
                    agent.as_ref().map(|scope| &scope.run_id),
                    client_key,
                ),
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
        let deadline = started_at
            .checked_add(descriptor.timeout_ms())
            .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::DeadlineElapsed))?;
        let mut record = ToolInvocationRecord {
            id: invocation_id.clone(),
            idempotency_key,
            session_id: self.session_id.clone(),
            turn_id: self.turn_id.clone(),
            agent_run_id: agent.as_ref().map(|scope| scope.run_id.clone()),
            workflow_run_id: self.workflow_run_id.clone(),
            workflow_step_id: self.workflow_step_id.clone(),
            project: self.project.clone(),
            capability_id,
            request: request.clone(),
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
                if existing.request != request
                    || existing.project != self.project
                    || existing.session_id != self.session_id
                {
                    return Err(CapabilityFailure::new(
                        CapabilityFailureCode::InvocationConflict,
                    ));
                }
                return self.recover_existing(*existing).await;
            }
        }

        let context = self
            .invocation_context(&record.idempotency_key)?
            .with_graph_observation(graph_observation.clone());
        let control = CapabilityControl::new(
            self.cancellation.clone(),
            Duration::from_millis(deadline.get().saturating_sub(self.clock.now().get())),
        );
        let started = self
            .emit(AgentEvent::ToolInvocationStarted {
                invocation_id: invocation_id.clone(),
                capability_id,
            })
            .await
            .and_then(|()| control.check());
        let mut citation = None;
        let mut outcome = match started {
            Err(error) => Err(error),
            Ok(()) => {
                // Catch adapter panics while polling so the authoritative ledger still closes.
                let invocation = async {
                    validation?;
                    let ToolInvocationRequest::Bound { request } = request else {
                        return Err(CapabilityFailure::new(
                            CapabilityFailureCode::InternalFailure,
                        ));
                    };
                    match request {
                        request @ (yss_harness_contract::AutomationCapabilityRequest::SearchKnowledge(_)
                        | yss_harness_contract::AutomationCapabilityRequest::ReadKnowledge(_)) => {
                            self.knowledge.invoke(request, &self.project, &control).await
                        }
                        request => self.gateway.invoke(context.clone(), request, control.clone()).await
                            .map(|result| (result, None)),
                    }
                };
                contain_tool_failure(invocation)
                    .await
                    .map(|(result, observed)| {
                        citation = observed;
                        result
                    })
            }
        };
        if outcome.as_ref().is_err_and(|failure| {
            matches!(
                failure.code,
                CapabilityFailureCode::InternalFailure | CapabilityFailureCode::OutcomeUnknown
            )
        }) && let Some(edit) = record.request.graph_edit()
            && let Ok(Some(receipt)) = self.gateway.recover_graph_edit(context, edit).await
        {
            outcome =
                Ok(yss_harness_contract::AutomationCapabilityResult::GraphEditReceipt(receipt));
        }
        if outcome
            .as_ref()
            .is_ok_and(|result| !result.accepts_capability(capability_id))
        {
            outcome = Err(CapabilityFailure::new(
                CapabilityFailureCode::InternalFailure,
            ));
        }
        if let (Some(scope), Ok(result)) = (&agent, &outcome)
            && let Err(failure) = crate::agents::validate_agent_read(scope, result)
        {
            outcome = Err(failure);
        }
        if let (Some(expected), Ok(result)) = (graph_observation.as_deref(), &outcome)
            && graph_read_inputs(result).is_some_and(|actual| actual != expected)
        {
            outcome = Err(
                CapabilityFailure::new(CapabilityFailureCode::RevisionConflict)
                    .with_detail("reason", "graph_inputs_changed"),
            );
        }
        // Owners bound mutation receipts before committing. Never turn an already
        // committed write into a retryable size failure or drop its created IDs.
        if descriptor.effect == ToolEffect::Inspect
            && outcome.as_ref().is_ok_and(|result| {
                result
                    .validate_budget(yss_harness_contract::MAX_CAPABILITY_RESULT_BYTES)
                    .is_err()
            })
        {
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
        outcome = outcome.map_err(|mut failure| {
            if matches!(
                failure.code,
                CapabilityFailureCode::RevisionConflict | CapabilityFailureCode::GraphDraftChanged
            ) && !failure.details.contains_key("resourceId")
                && let Some(resource) = record.request.bound().and_then(request_resource_id)
            {
                failure = failure.with_detail("resourceId", resource);
            }
            failure.with_detail("capabilityId", capability_id.as_str())
        });
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
        // Business commit is authoritative. A storage/display failure cannot undo it;
        // a retained Running record is repaired by querying the same operation identity.
        let committed = descriptor.effect != ToolEffect::Inspect && outcome.is_ok();
        if self.ledger.finish(&record).await.is_err() && !committed {
            return Err(persistence_unavailable());
        }

        let delivery = self
            .emit(match &outcome {
                Ok(_) => AgentEvent::ToolInvocationCompleted {
                    invocation_id: invocation_id.clone(),
                    capability_id,
                },
                Err(failure) => AgentEvent::ToolInvocationFailed {
                    invocation_id: invocation_id.clone(),
                    capability_id,
                    failure_code: failure.code,
                    failure_details: Some(failure.details.clone()),
                },
            })
            .await;
        if !committed {
            delivery?;
        }
        if let Ok(yss_harness_contract::AutomationCapabilityResult::GraphExecution(execution)) =
            &outcome
        {
            self.emit(AgentEvent::GraphExecutionFinished {
                invocation_id: invocation_id.clone(),
                status: execution.status.clone(),
                failure_code: execution.failure_code.clone(),
            })
            .await?;
        }

        if outcome.is_ok()
            && let Some(citation) = citation
        {
            self.emit(AgentEvent::KnowledgeCited { citation }).await?;
        }
        outcome.map(|result| ModelCapabilityOutcome {
            invocation_id,
            result,
        })
    }

    fn invocation_context(
        &self,
        key: &IdempotencyKey,
    ) -> Result<CapabilityInvocationContext, CapabilityFailure> {
        let mut context = CapabilityInvocationContext::new(
            self.principal_id.clone(),
            self.session_id.clone(),
            CapabilityInvocationId::try_new(key.as_str()).map_err(|_| persistence_unavailable())?,
            self.project.clone(),
        );
        if let Some(agent) = &self.agent {
            context = context.with_agent(agent.lock().unwrap_or_else(|e| e.into_inner()).clone());
        }
        Ok(match &self.approval_grant_id {
            Some(grant_id) => context.with_approval(grant_id.clone()),
            None => context,
        })
    }

    async fn recover_existing(
        &self,
        mut record: ToolInvocationRecord,
    ) -> Result<ModelCapabilityOutcome, CapabilityFailure> {
        if record.result.is_some()
            || record
                .failure
                .as_ref()
                .is_some_and(|failure| failure.code != CapabilityFailureCode::OutcomeUnknown)
        {
            return replay_existing(record);
        }
        if let Some(edit) = record.request.graph_edit() {
            let context = self.invocation_context(&record.idempotency_key)?;
            if let Some(receipt) = self.gateway.recover_graph_edit(context, edit).await? {
                record.result = Some(
                    yss_harness_contract::AutomationCapabilityResult::GraphEditReceipt(receipt),
                );
                record.failure = None;
                record.state = ToolInvocationState::Succeeded;
                record.finished_at = Some(self.clock.now());
                let _ = self.ledger.finish(&record).await;
                return replay_existing(record);
            }
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::OutcomeUnknown,
            ));
        }
        replay_existing(record)
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

pub(crate) async fn contain_tool_failure<T>(
    future: impl Future<Output = Result<T, CapabilityFailure>>,
) -> Result<T, CapabilityFailure> {
    let mut future = std::pin::pin!(future);
    poll_fn(
        |context| match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(context))) {
            Ok(result) => result,
            Err(_) => Poll::Ready(Err(CapabilityFailure::new(
                CapabilityFailureCode::InternalFailure,
            ))),
        },
    )
    .await
}

fn request_resource_id(request: &AutomationCapabilityRequest) -> Option<&str> {
    use AutomationCapabilityRequest::*;
    use yss_harness_contract::ManageResourceRequest as Manage;
    Some(match request {
        InspectChart(value) => &value.chart.id,
        InspectResource(value) => &value.resource.id,
        ManageResource(
            Manage::Rename { resource, .. }
            | Manage::Duplicate { resource, .. }
            | Manage::Delete { resource, .. }
            | Manage::Save { resource, .. },
        ) => &resource.id,
        EditResource(value) => &value.resource.id,
        ReadDatabase(value) => &value.input.database().id,
        ReadMind(value) => &value.input.mind().id,
        ReadDocument(value) => &value.input.document().id,
        ExportDatabase(value) => &value.resource.id,
        InspectDatasetSchema(value) => &value.database_id,
        InspectDatasetProfile(value) => &value.database_id,
        InspectGraph(value) => &value.graph.id,
        FindNodes(value) => &value.graph.id,
        FindConstants(value) => &value.graph.id,
        InspectConstants(value) => &value.graph.id,

        InspectNodes(value) => &value.graph.id,
        FindConnections(value) => &value.graph.id,
        ApplyGraphEdit(value) => &value.graph_path,
        GraphMutation(value) => &value.input.graph().id,
        ValidateGraph(value) => &value.graph.id,
        ExecuteGraph(value) => &value.graph.id,
        SaveGraph(value) => &value.graph_path,
        ListGraphResults(value) => &value.graph.id,
        _ => return None,
    })
}

fn graph_read_inputs(result: &yss_harness_contract::AutomationCapabilityResult) -> Option<&str> {
    use yss_harness_contract::AutomationCapabilityResult as Result;
    match result {
        Result::GraphInspection(value) => Some(&value.semantic_input_hash),
        Result::GraphInspectionPage(value) => Some(&value.semantic_input_hash),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FixedClock, InMemoryHarnessStore, SequentialIds};
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use yss_harness_contract::{
        AgentFuture, AgentOutputFailure, AutomationCapabilityRequest, CancellationReason,
        CapabilityFuture, DatabaseReadRequest, model,
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
                    4 => panic!("invalid arguments must not reach the gateway"),
                    5 => {
                        return Err(CapabilityFailure::new(
                            CapabilityFailureCode::RevisionConflict,
                        ));
                    }
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
            (4, CapabilityFailureCode::InvalidRequest),
            (5, CapabilityFailureCode::RevisionConflict),
            (6, CapabilityFailureCode::InvalidRequest),
            (7, CapabilityFailureCode::InvalidRequest),
        ] {
            let store = Arc::new(InMemoryHarnessStore::default());
            let clock = Arc::new(FixedClock::new(1000));
            let output = Arc::new(Output::default());
            let executor = HarnessToolExecutor::new(
                ToolRegistry::for_agent(yss_harness_contract::AgentRole::Review),
                Arc::new(FailingGateway(mode, clock.clone())),
                Arc::new(crate::KnowledgeService::new(
                    store.clone(),
                    Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
                )),
                store.clone(),
                clock.clone(),
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
            let request = if mode == 4 {
                AutomationCapabilityRequest::ReadDocument(yss_harness_contract::DocumentReadRequest {
                    input:model::DocumentReadInput::Text(serde_json::from_value(serde_json::json!({"document":{"kind":"doc","id":"docs/report.md"},"limit":0})).unwrap()),
                    version:None,
                })
            } else {
                AutomationCapabilityRequest::ReadDatabase(DatabaseReadRequest {
                    input: model::DatabaseReadInput::Overview(model::InspectDatabaseInput {
                        database: model::DatabaseResourceRef::new("data-1".into()),
                    }),
                    version: None,
                })
            };
            let failure = if mode >= 6 {
                let started_at = clock.now();
                clock.advance(37);
                executor
                    .reject(
                        request.capability_id(),
                        (mode == 7).then(|| (&request).into()),
                        CapabilityFailure::new(CapabilityFailureCode::InvalidRequest),
                        started_at,
                    )
                    .await
            } else {
                executor.execute(request, None).await
            }
            .unwrap_err();
            assert_eq!(failure.code, expected);
            if mode == 4 {
                let visible = yss_harness_contract::model::failure(&failure);
                assert_eq!(visible["details"]["field"], "limit");
                assert_eq!(visible["details"]["maximumResults"], "16384");
            } else if mode == 5 {
                let visible = yss_harness_contract::model::failure(&failure);
                assert_eq!(visible["code"], "resource_changed");
                assert_eq!(visible["details"]["resourceId"], "data-1");
            }
            let records = store.tool_invocations();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].state, ToolInvocationState::Failed);
            assert_eq!(records[0].failure.as_ref().unwrap().code, expected);
            assert!(records[0].finished_at.is_some());
            if mode >= 6 {
                assert!(records[0].request.bound().is_none());
                assert_eq!(
                    records[0].finished_at.unwrap().get() - records[0].started_at.get(),
                    37
                );
                assert_eq!(records[0].request.model_input().is_some(), mode == 7);
                assert_eq!(
                    records[0].request.capability_id(),
                    CapabilityId::InspectDatabase
                );
            }
            assert!(store.load_running_invocations().await.unwrap().is_empty());
            assert!(matches!(output.0.lock().unwrap().as_slice(), [
                AgentEvent::ToolInvocationStarted { invocation_id: started, .. },
                AgentEvent::ToolInvocationFailed { invocation_id: finished, failure_code, failure_details, .. },
            ] if started == finished && started == &records[0].id && *failure_code == expected
                && failure_details.as_ref() == Some(&failure.details)));
            if mode == 2 {
                // Work queued before cancellation must not begin a new plan or delegation.
                let tool = yss_harness_contract::AgentControlTool::ProposeStatisticalPlan;
                assert_eq!(
                    executor.begin_control(tool).await.unwrap_err().code,
                    CapabilityFailureCode::Cancelled
                );
                assert!(matches!(&output.0.lock().unwrap()[2..], [
                    AgentEvent::ControlToolStarted { invocation_id: started, .. },
                    AgentEvent::ControlToolFinished { invocation_id: finished, failure_code: Some(CapabilityFailureCode::Cancelled), .. },
                ] if started == finished));
            } else if mode == 6 {
                let tool = yss_harness_contract::AgentControlTool::DelegateTask;
                let id = executor.begin_control(tool).await.unwrap();
                let failure = CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
                    .with_detail("category", "missing_field")
                    .with_detail("path", "$.constraints")
                    .with_detail("expected", "type=string");
                executor
                    .finish_control(id.clone(), tool, Some(failure.clone()))
                    .await
                    .unwrap();
                assert!(matches!(&output.0.lock().unwrap()[2..], [
                    AgentEvent::ControlToolStarted { invocation_id: started, .. },
                    AgentEvent::ControlToolFinished {
                        invocation_id: finished,
                        failure_code: Some(CapabilityFailureCode::InvalidRequest),
                        failure_details: Some(details), ..
                    },
                ] if started == &id && finished == &id && details == &failure.details));
            }
        }
    }

    #[test]
    fn review_registry_is_read_only_and_stats_registers_its_graph_tools() {
        let registry = ToolRegistry::for_agent(yss_harness_contract::AgentRole::Review);

        assert!(registry.descriptor(CapabilityId::InspectGraph).is_some());
        assert!(registry.descriptor(CapabilityId::InspectResource).is_some());
        assert!(registry.descriptor(CapabilityId::CreateResource).is_none());
        assert!(registry.descriptor(CapabilityId::ApplyGraphEdit).is_none());
        let step = ToolRegistry::for_capability(CapabilityId::InspectDatasetSchema);
        assert!(
            step.descriptor(CapabilityId::InspectDatasetSchema)
                .is_some()
        );
        assert!(step.descriptor(CapabilityId::ApplyGraphEdit).is_none());
        let editor = ToolRegistry::for_agent(yss_harness_contract::AgentRole::Stats);
        assert!(editor.descriptor(CapabilityId::ApplyGraphEdit).is_none());
        assert!(editor.descriptor(CapabilityId::SaveGraph).is_none());
        for id in [
            CapabilityId::InspectResource,
            CapabilityId::CreateResource,
            CapabilityId::EditResource,
            CapabilityId::CreateNodes,
            CapabilityId::UndoResource,
            CapabilityId::RedoResource,
            CapabilityId::ValidateGraph,
            CapabilityId::ExecuteGraph,
            CapabilityId::SaveResource,
            CapabilityId::ListGraphResults,
        ] {
            assert!(editor.descriptor(id).is_some());
        }
    }

    #[tokio::test]
    async fn graph_edit_retries_reuse_the_receipt_and_reject_changed_requests_with_the_same_key() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use yss_harness_contract::{
            ApplyGraphEditRequest, AutomationCapabilityResult, GraphEditPosition, GraphEditReceipt,
            GraphMutationRequest, GraphResourceRef, PersistenceFailure, PersistenceFailureCode,
            PersistenceFuture,
        };
        struct Gateway {
            writes: AtomicUsize,
            reads: AtomicUsize,
            committed: Mutex<Option<(CapabilityInvocationContext, GraphEditReceipt)>>,
        }
        impl CapabilityGatewayPort for Gateway {
            fn invoke<'a>(
                &'a self,
                context: CapabilityInvocationContext,
                _: AutomationCapabilityRequest,
                control: CapabilityControl,
            ) -> CapabilityFuture<'a> {
                Box::pin(async move {
                    self.writes.fetch_add(1, Ordering::SeqCst);
                    let receipt = GraphEditReceipt {
                        created_constants: Default::default(),
                        graph_path: "events/Main.yssbi-event".into(),
                        from_revision: 0,
                        to_revision: 1,
                        client_key: "batch-1".into(),
                        graph_hash: "1".repeat(64),
                        created_nodes: [(
                            "large-committed-mapping".into(),
                            "n".repeat(yss_harness_contract::MAX_CAPABILITY_RESULT_BYTES + 1),
                        )]
                        .into_iter()
                        .collect(),
                        created_ports: BTreeMap::new(),
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
                    };
                    *self.committed.lock().unwrap() = Some((context, receipt.clone()));
                    control.cancellation().cancel(CancellationReason::User);
                    Ok(AutomationCapabilityResult::GraphEditReceipt(receipt))
                })
            }
            fn recover_graph_edit<'a>(
                &'a self,
                context: CapabilityInvocationContext,
                _: ApplyGraphEditRequest,
            ) -> AgentFuture<'a, Result<Option<GraphEditReceipt>, CapabilityFailure>> {
                Box::pin(async move {
                    self.reads.fetch_add(1, Ordering::SeqCst);
                    let committed = self.committed.lock().unwrap();
                    let (original, receipt) = committed.as_ref().unwrap();
                    assert_eq!(context.invocation_id(), original.invocation_id());
                    assert_eq!(context.project(), original.project());
                    Ok(Some(receipt.clone()))
                })
            }
        }
        struct FailFirstFinish(Arc<InMemoryHarnessStore>, AtomicBool);
        impl ToolInvocationLedgerPort for FailFirstFinish {
            fn load_invocation<'a>(
                &'a self,
                session: &'a HarnessSessionId,
                id: &'a ToolInvocationId,
            ) -> PersistenceFuture<'a, Result<Option<ToolInvocationRecord>, PersistenceFailure>>
            {
                self.0.load_invocation(session, id)
            }

            fn load_running_invocations<'a>(
                &'a self,
            ) -> PersistenceFuture<'a, Result<Vec<ToolInvocationRecord>, PersistenceFailure>>
            {
                self.0.load_running_invocations()
            }
            fn begin<'a>(
                &'a self,
                record: &'a ToolInvocationRecord,
            ) -> PersistenceFuture<'a, Result<ToolInvocationBegin, PersistenceFailure>>
            {
                self.0.begin(record)
            }
            fn finish<'a>(
                &'a self,
                record: &'a ToolInvocationRecord,
            ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
                if self.1.swap(false, Ordering::SeqCst) {
                    Box::pin(async {
                        Err(PersistenceFailure::new(PersistenceFailureCode::Unavailable))
                    })
                } else {
                    self.0.finish(record)
                }
            }
        }
        struct ClosedAfterStart;
        impl AgentEventOutput for ClosedAfterStart {
            fn emit<'a>(
                &'a self,
                event: AgentEvent,
            ) -> AgentFuture<'a, Result<(), AgentOutputFailure>> {
                Box::pin(async move {
                    if matches!(event, AgentEvent::ToolInvocationStarted { .. }) {
                        Ok(())
                    } else {
                        Err(AgentOutputFailure::Closed)
                    }
                })
            }
        }
        let gateway = Arc::new(Gateway {
            writes: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
            committed: Mutex::new(None),
        });
        let store = Arc::new(InMemoryHarnessStore::default());
        let executor = HarnessToolExecutor::new(
            ToolRegistry::for_agent(yss_harness_contract::AgentRole::Stats),
            gateway.clone(),
            Arc::new(crate::KnowledgeService::new(
                store.clone(),
                Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
            )),
            Arc::new(FailFirstFinish(store.clone(), AtomicBool::new(true))),
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
        )
        .with_output(Arc::new(ClosedAfterStart));
        let mut edit = GraphMutationRequest {
            graph_hash: "0".repeat(64),
            base_revision: 0,
            client_key: "batch-1".into(),
            input: yss_harness_contract::model::GraphMutationInput::MoveNodes(
                yss_harness_contract::model::MoveNodesInput {
                    graph: GraphResourceRef::for_path("events/Main.yssbi-event"),
                    positions: vec![GraphEditPosition {
                        node_id: "node-1".into(),
                        x: 1.,
                        y: 2.,
                    }],
                },
            ),
        };
        let first = executor
            .execute(
                AutomationCapabilityRequest::GraphMutation(edit.clone()),
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            store.tool_invocations()[0].state,
            ToolInvocationState::Running
        );
        let retry = executor
            .execute(
                AutomationCapabilityRequest::GraphMutation(edit.clone()),
                None,
            )
            .await
            .unwrap();
        assert_eq!(first, retry);
        assert_eq!(
            store.tool_invocations()[0].state,
            ToolInvocationState::Succeeded
        );
        let retained = executor
            .execute(
                AutomationCapabilityRequest::GraphMutation(edit.clone()),
                None,
            )
            .await
            .unwrap();
        assert_eq!(retained, first);
        edit.base_revision = 1;
        let conflict = executor
            .execute(AutomationCapabilityRequest::GraphMutation(edit), None)
            .await
            .unwrap_err();
        assert_eq!(conflict.code, CapabilityFailureCode::InvocationConflict);
        assert_eq!(gateway.writes.load(Ordering::SeqCst), 1);
        assert_eq!(gateway.reads.load(Ordering::SeqCst), 1);
        assert_eq!(store.tool_invocations().len(), 1);
    }
}
