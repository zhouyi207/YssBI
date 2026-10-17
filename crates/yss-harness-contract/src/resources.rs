//! Explicit user-selected project resource references attached to a turn.

use crate::{CapabilityContractError, validate_resource_id};
use serde::{Deserialize, Serialize};
pub use yss_project_identity::{ProjectResourceKind, ProjectResourceRef};

/// An explicit user-selected target. Contents remain with their resource owner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarnessResourceReference {
    pub resource: ProjectResourceRef,
    pub name: String,
}

impl HarnessResourceReference {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("resource.id", &self.resource.id)
    }
}

/// Resolve explicit references through the project owner during turn preparation.
pub trait HarnessResourceResolverPort: Send + Sync {
    fn resolve<'a>(
        &'a self,
        project: &'a crate::ProjectSessionBinding,
        resources: &'a [ProjectResourceRef],
        cancellation: crate::CancellationToken,
    ) -> crate::AgentFuture<'a, Result<Vec<HarnessResourceReference>, crate::CapabilityFailure>>;
}
