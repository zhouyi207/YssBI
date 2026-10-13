use crate::events::PersistingAgentOutput;
use crate::tools::{HarnessToolExecutor, InvocationScope, ToolRegistry};
use crate::{ApprovalService, HarnessError, HarnessHost};
use serde_json::Value;
use std::sync::Arc;
use yss_harness_contract::{
    ApprovalGrantId, ApprovalGrantRecord, CancellationToken, CapabilityFailure,
    CapabilityFailureCode, HarnessSessionId, HarnessSessionRecord, HarnessSessionState,
    HarnessTurnId, PreparedToolCall, ToolSpec,
};

impl HarnessHost {
    pub async fn issue_capability_approval(
        &self,
        session_id: &HarnessSessionId,
        turn_id: &HarnessTurnId,
        tool: &str,
        request: &Value,
        ttl_ms: u64,
    ) -> Result<ApprovalGrantRecord, HarnessError> {
        let session = self.approval_session(session_id, turn_id).await?;
        let spec = self.approval_tool(tool, request)?;
        Ok(ApprovalService::new(
            Arc::clone(&self.ports.approvals),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .issue(
            session.principal_id,
            session.id,
            session.project,
            &spec,
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
        tool: &str,
        request: Value,
    ) -> Result<Value, HarnessError> {
        let session = self.approval_session(session_id, turn_id).await?;
        let spec = self.approval_tool(tool, &request)?;
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
            tool,
            &request,
        )
        .await?;
        let output = Arc::new(PersistingAgentOutput {
            run_id: None,
            writer: self.event_writer(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
        });
        let executor = HarnessToolExecutor::new(
            ToolRegistry::new([spec]),
            self.tool_services(),
            InvocationScope {
                principal_id: session.principal_id,
                session_id: session.id,
                turn_id: turn_id.clone(),
                project: session.project,
            },
            CancellationToken::default(),
        )
        .approved(approval_grant_id.clone())
        .with_output(output);
        let call = PreparedToolCall {
            tool: tool.to_owned(),
            arguments: self.ports.extension.project_arguments(tool, &request),
            request,
            idempotency_token: None,
            internal: true,
        };
        let outcome = executor.execute(call, executor.now()).await?;
        Ok(outcome.result)
    }

    async fn approval_session(
        &self,
        session_id: &HarnessSessionId,
        turn_id: &HarnessTurnId,
    ) -> Result<HarnessSessionRecord, HarnessError> {
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
        Ok(session)
    }

    fn approval_tool(&self, tool: &str, request: &Value) -> Result<ToolSpec, HarnessError> {
        let spec = self.ports.extension.tool_spec(tool).ok_or_else(|| {
            CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                .with_detail("capabilityId", tool)
        })?;
        self.ports.extension.validate_request(tool, request)?;
        Ok(spec)
    }
}
