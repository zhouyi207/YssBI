//! Automation identities, project bindings and the generic invocation identity.

use crate::AgentRunId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

/// Declares validation, accessors and serde for a bounded string identity.
#[doc(hidden)]
#[macro_export]
macro_rules! string_identity {
    ($name:ident, $label:literal) => {
        impl $name {
            pub fn try_new(
                value: impl Into<String>,
            ) -> Result<Self, $crate::AutomationIdentityError> {
                let value = value.into();
                if value.trim().is_empty() || value.len() > 128 {
                    return Err($crate::AutomationIdentityError::Invalid($label));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
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

/// The Harness-owned identity of one tool invocation. Extensions add their own call bindings.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolInvocationContext {
    pub principal_id: PrincipalId,
    pub harness_session_id: HarnessSessionId,
    pub invocation_id: CapabilityInvocationId,
    pub project: ProjectSessionBinding,
    pub agent_run_id: Option<AgentRunId>,
}

impl ToolInvocationContext {
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
            agent_run_id: None,
        }
    }
}
