//! Automation identities, project bindings and the invocation authority envelope.

use crate::{AgentInvocationScope, ApprovalGrantId};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

macro_rules! string_identity {
    ($name:ident, $label:literal) => {
        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, AutomationIdentityError> {
                let value = value.into();
                if value.trim().is_empty() || value.len() > 128 {
                    return Err(AutomationIdentityError::Invalid($label));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::try_new(value).map_err(serde::de::Error::custom)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct PrincipalId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct HarnessSessionId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct CapabilityInvocationId(String);

string_identity!(PrincipalId, "principal id");
string_identity!(HarnessSessionId, "harness session id");
string_identity!(CapabilityInvocationId, "capability invocation id");

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AutomationIdentityError {
    #[error("invalid {0}")]
    Invalid(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSessionBinding {
    project_instance_id: ProjectInstanceId,
    project_session_id: ProjectSessionId,
}

impl ProjectSessionBinding {
    pub fn new(
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
    ) -> Self {
        Self {
            project_instance_id,
            project_session_id,
        }
    }

    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn project_session_id(&self) -> &ProjectSessionId {
        &self.project_session_id
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityInvocationContext {
    principal_id: PrincipalId,
    harness_session_id: HarnessSessionId,
    invocation_id: CapabilityInvocationId,
    project: ProjectSessionBinding,
    approval_grant_id: Option<ApprovalGrantId>,
    agent: Option<AgentInvocationScope>,
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

    pub fn with_approval(mut self, approval_grant_id: ApprovalGrantId) -> Self {
        self.approval_grant_id = Some(approval_grant_id);
        self
    }

    pub fn approval_grant_id(&self) -> Option<&ApprovalGrantId> {
        self.approval_grant_id.as_ref()
    }

    pub fn with_agent(mut self, agent: AgentInvocationScope) -> Self {
        self.agent = Some(agent);
        self
    }

    pub fn agent(&self) -> Option<&AgentInvocationScope> {
        self.agent.as_ref()
    }
}
