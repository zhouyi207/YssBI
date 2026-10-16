//! The Application authority envelope for one Assistant capability invocation.

use crate::{
    AgentInvocationScope, ApprovalGrantId, CapabilityInvocationId, HarnessSessionId, PrincipalId,
    ProjectSessionBinding, ToolInvocationContext,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityInvocationContext {
    principal_id: PrincipalId,
    harness_session_id: HarnessSessionId,
    invocation_id: CapabilityInvocationId,
    project: ProjectSessionBinding,
    approval_grant_id: Option<ApprovalGrantId>,
    agent: Option<AgentInvocationScope>,
    graph_observation: Option<String>,
}

impl CapabilityInvocationContext {
    pub fn new(
        principal_id: PrincipalId,
        harness_session_id: HarnessSessionId,
        invocation_id: CapabilityInvocationId,
        project: ProjectSessionBinding,
    ) -> Self {
        Self {
            principal_id,
            harness_session_id,
            invocation_id,
            project,
            approval_grant_id: None,
            agent: None,
            graph_observation: None,
        }
    }

    pub fn principal_id(&self) -> &PrincipalId {
        &self.principal_id
    }

    pub fn harness_session_id(&self) -> &HarnessSessionId {
        &self.harness_session_id
    }

    pub fn invocation_id(&self) -> &CapabilityInvocationId {
        &self.invocation_id
    }

    pub fn project(&self) -> &ProjectSessionBinding {
        &self.project
    }

    pub fn with_agent(mut self, agent: AgentInvocationScope) -> Self {
        self.agent = Some(agent);
        self
    }

    pub fn agent(&self) -> Option<&AgentInvocationScope> {
        self.agent.as_ref()
    }

    /// Captured semantic inputs for this graph call, never a model argument.
    pub fn with_graph_observation(mut self, hash: Option<String>) -> Self {
        self.graph_observation = hash;
        self
    }

    pub fn graph_observation(&self) -> Option<&str> {
        self.graph_observation.as_deref()
    }
}

impl From<ToolInvocationContext> for CapabilityInvocationContext {
    fn from(context: ToolInvocationContext) -> Self {
        let ToolInvocationContext {
            principal_id,
            harness_session_id,
            invocation_id,
            project,
            approval_grant_id,
            agent_run_id: _,
        } = context;
        Self {
            approval_grant_id,
            ..Self::new(principal_id, harness_session_id, invocation_id, project)
        }
    }
}
