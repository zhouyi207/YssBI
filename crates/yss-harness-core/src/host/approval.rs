use crate::events::PersistingAgentOutput;
use crate::tools::HarnessToolExecutor;
use crate::{ApprovalService, HarnessError, HarnessHost, ToolRegistry};
use std::sync::Arc;
use yss_harness_contract::{
    ApprovalGrantId, ApprovalGrantRecord, AutomationCapabilityRequest, AutomationCapabilityResult,
    HarnessSessionId, HarnessSessionState, HarnessTurnId,
};

impl HarnessHost {
    pub async fn issue_capability_approval(
        &self,
        session_id: &HarnessSessionId,
        turn_id: &HarnessTurnId,
        request: &AutomationCapabilityRequest,
        ttl_ms: u64,
    ) -> Result<ApprovalGrantRecord, HarnessError> {
        let session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active {
            return Err(HarnessError::SessionNotActive);
        }
        let turn = self
            .ports
            .sessions
            .load_turn(turn_id)
            .await?
            .ok_or(HarnessError::WorkflowTurnNotFound)?;
        if turn.session_id != session.id {
            return Err(HarnessError::WorkflowTurnMismatch);
        }
        Ok(ApprovalService::new(
            Arc::clone(&self.ports.approvals),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .issue(
            session.principal_id,
            session.id,
            session.project,
            request,
            ttl_ms,
        )
        .await?)
    }

    pub async fn execute_approved_capability(
        &self,
        session_id: &HarnessSessionId,
        turn_id: &HarnessTurnId,
        approval_grant_id: &ApprovalGrantId,
        request: AutomationCapabilityRequest,
    ) -> Result<AutomationCapabilityResult, HarnessError> {
        let session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active {
            return Err(HarnessError::SessionNotActive);
        }
        let turn = self
            .ports
            .sessions
            .load_turn(turn_id)
            .await?
            .ok_or(HarnessError::WorkflowTurnNotFound)?;
        if turn.session_id != session.id {
            return Err(HarnessError::WorkflowTurnMismatch);
        }
        ApprovalService::new(
            Arc::clone(&self.ports.approvals),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .consume(
            approval_grant_id,
            &session.principal_id,
            &session.id,
            &session.project,
            &request,
        )
        .await?;
        let capability_id = request.capability_id();
        let output = Arc::new(PersistingAgentOutput {
            run_id: None,
            writer: self.event_writer(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
        });
        let registry = ToolRegistry::for_capability(capability_id);
        let outcome = HarnessToolExecutor::new_approved(
            registry,
            Arc::clone(&self.ports.capability_gateway),
            Arc::clone(&self.knowledge),
            Arc::clone(&self.ports.tool_ledger),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
            session.principal_id,
            session.id,
            turn_id.clone(),
            session.project,
            approval_grant_id.clone(),
        )
        .with_output(output)
        .execute(request, None)
        .await?;
        Ok(outcome.result)
    }
}
