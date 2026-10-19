//! YssBI Manager–Worker roles, task authority and receipt-derived task facts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    AgentRoleId, AgentRunId, CapabilityContractError, GraphResultReference, ProjectResourceRef,
    ResourceChange, ResourceCreation, ResourceVersion, StatisticalPlan,
};

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Manager,
    Data,
    Stats,
    Plot,
    Report,
    Review,
}

impl AgentRole {
    pub const ALL: [Self; 6] = [
        Self::Manager,
        Self::Data,
        Self::Stats,
        Self::Plot,
        Self::Report,
        Self::Review,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manager => "manager",
            Self::Data => "data",
            Self::Stats => "stats",
            Self::Plot => "plot",
            Self::Report => "report",
            Self::Review => "review",
        }
    }

    pub fn id(self) -> AgentRoleId {
        AgentRoleId::try_new(self.as_str()).expect("built-in role ids are valid")
    }

    pub fn from_id(id: &AgentRoleId) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|role| role.as_str() == id.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentResourceVersion {
    pub resource: ProjectResourceRef,
    pub version: ResourceVersion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentResourceOperation {
    Inspect,
    Edit,
    Rename,
    Duplicate,
    Delete,
    Save,
    Execute,
    Export,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentResourceAccess {
    pub resource: ProjectResourceRef,
    pub version: Option<ResourceVersion>,
    pub operations: Vec<AgentResourceOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentResultAccess {
    pub result_ref: crate::ResultRef,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTaskScope {
    pub resources: Vec<AgentResourceAccess>,
    pub results: Vec<AgentResultAccess>,
    pub creations: Vec<AgentCreationAccess>,
    pub export_paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentCreationAccess {
    pub specification: ResourceCreation,
    pub operations: Vec<AgentResourceOperation>,
}

impl AgentTaskScope {
    pub fn is_read_only(&self) -> bool {
        self.creations.is_empty()
            && self.export_paths.is_empty()
            && self.resources.iter().all(|access| {
                access
                    .operations
                    .iter()
                    .all(|op| *op == AgentResourceOperation::Inspect)
            })
    }

    pub fn validate(&self, worker: AgentRole) -> Result<(), CapabilityContractError> {
        if worker == AgentRole::Manager || (worker == AgentRole::Review && !self.is_read_only()) {
            return Err(CapabilityContractError::InvalidField("task"));
        }
        for access in &self.resources {
            crate::validate_resource_id("resourceId", &access.resource.id)?;
            if access.version.is_none() {
                return Err(CapabilityContractError::InvalidField(
                    "scope.resources.version",
                ));
            }
            if !access.operations.contains(&AgentResourceOperation::Inspect)
                || access.operations.len() > 8
            {
                return Err(CapabilityContractError::InvalidField("operations"));
            }
        }
        for creation in &self.creations {
            crate::ManageResourceRequest::Create {
                specification: creation.specification.clone(),
            }
            .validate()?;
            if !creation
                .operations
                .contains(&AgentResourceOperation::Inspect)
                || creation.operations.len() > 8
            {
                return Err(CapabilityContractError::InvalidField("creation.operations"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentInvocationScope {
    pub run_id: AgentRunId,
    pub role: AgentRole,
    /// Only the Manager has project-wide discovery. Workers always receive a scope.
    pub task: Option<AgentTaskScope>,
}

/// Facts collected from actual tool receipts and stored as the Harness outcome's facts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTaskFacts {
    pub artifacts: Vec<ResourceChange>,
    pub results: Vec<GraphResultReference>,
    pub plan: Option<StatisticalPlan>,
}

impl AgentTaskFacts {
    /// Null facts belong to runs that recorded no receipts.
    pub fn from_value(value: &serde_json::Value) -> Result<Self, serde_json::Error> {
        if value.is_null() {
            return Ok(Self::default());
        }
        serde_json::from_value(value.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentFollowupBindings {
    pub resource_versions: Vec<AgentResourceVersion>,
}
