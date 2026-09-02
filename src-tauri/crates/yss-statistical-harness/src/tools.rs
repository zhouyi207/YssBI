use std::collections::BTreeMap;
use std::sync::Arc;

use yss_automation_contract::{
    ApprovalGrantId, AutomationIdKind, AutomationIdentityError, CancellationToken,
    CapabilityFailure, CapabilityFailureCode, CapabilityGatewayPort, CapabilityId,
    CapabilityInvocationContext, CapabilityInvocationId, ClockPort, HarnessSessionId,
    HarnessTurnId, IdGeneratorPort, IdempotencyKey, ModelCapabilityExecutor,
    ModelCapabilityOutcome, ModelCapabilityRequest, PrincipalId, ProjectSessionBinding,
    ToolDescriptor, ToolInvocationBegin, ToolInvocationId, ToolInvocationLedgerPort,
    ToolInvocationRecord, ToolInvocationState, WorkflowRunId, WorkflowStepId,
};

#[derive(Clone, Debug)]
pub struct ToolRegistry {
    descriptors: BTreeMap<CapabilityId, ToolDescriptor>,
}

impl ToolRegistry {
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
        }
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
            IdempotencyKey::try_new(raw_invocation_id).map_err(|_| persistence_unavailable())?;
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
        let mut outcome = self.gateway.invoke(context, request.request).await;
        if outcome
            .as_ref()
            .is_ok_and(|result| result.capability_id() != capability_id)
        {
            outcome = Err(CapabilityFailure::new(
                CapabilityFailureCode::InternalFailure,
            ));
        }
        if self.cancellation.is_cancelled() {
            outcome = Err(CapabilityFailure::new(CapabilityFailureCode::Cancelled));
        } else if self.clock.now() > deadline {
            outcome = Err(CapabilityFailure::new(
                CapabilityFailureCode::DeadlineElapsed,
            ));
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

    #[test]
    fn default_registry_keeps_mutating_capabilities_unrouted() {
        let registry = ToolRegistry::read_only_foundation().unwrap();

        assert!(registry.descriptor(CapabilityId::InspectGraph).is_some());
        assert!(registry.descriptor(CapabilityId::ApplyGraphEdit).is_none());
    }
}
