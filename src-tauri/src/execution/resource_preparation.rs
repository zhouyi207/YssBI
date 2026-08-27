use std::collections::BTreeSet;

use thiserror::Error;

use crate::execution::plan::{
    PlanCompilationBasis, PlanResourceId, PlanResourceRequirement, PlanResourceVersion,
    ResourceAccess, ResourceKind,
};

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ResourcePreparationError {
    #[error("resource requirement is duplicated")]
    DuplicateRequirement,
    #[error("resource requirement is missing a version observation")]
    MissingVersion { resource: PlanResourceId },
    #[error("resource requirement is unavailable")]
    Unavailable { resource: PlanResourceId },
    #[error("resource requirement is invalid")]
    InvalidRequirement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SealedResourceGrant {
    resource: PlanResourceId,
    version: PlanResourceVersion,
    kind: ResourceKind,
    access: ResourceAccess,
}

#[derive(Debug)]
struct PreparedRunResources {
    grants: Box<[SealedResourceGrant]>,
    effect_buffer: ExecutionEffectBuffer,
}

#[derive(Debug, Default)]
struct ExecutionEffectBuffer {
    effects: Vec<ExecutionEffect>,
}

#[derive(Debug)]
enum ExecutionEffect {
    VariableWrite { resource: PlanResourceId },
    PlotCandidate { resource: PlanResourceId },
}

impl PreparedRunResources {
    fn new(grants: Box<[SealedResourceGrant]>) -> Self {
        Self {
            grants,
            effect_buffer: ExecutionEffectBuffer::default(),
        }
    }

    fn finish(self) -> Box<[PlanResourceId]> {
        self.grants
            .into_vec()
            .into_iter()
            .map(|grant| grant.resource)
            .collect()
    }
}

#[derive(Clone)]
pub struct ResourceProviderFactory {
    session_identity: Box<str>,
}

impl ResourceProviderFactory {
    pub(crate) fn new(session_identity: Box<str>) -> Self {
        Self { session_identity }
    }

    fn prepare(
        &self,
        requirements: &[PlanResourceRequirement],
        basis: &PlanCompilationBasis,
    ) -> Result<PreparedRunResources, ResourcePreparationError> {
        if self.session_identity.as_ref() != basis.project_session().as_str() {
            return Err(ResourcePreparationError::InvalidRequirement);
        }
        let mut seen = BTreeSet::new();
        let grants = requirements
            .iter()
            .map(|requirement| {
                if !seen.insert(requirement.resource().clone()) {
                    return Err(ResourcePreparationError::DuplicateRequirement);
                }
                let Some(version) = basis.resource_versions().get(requirement.resource()) else {
                    if requirement.optional() {
                        return Err(ResourcePreparationError::Unavailable {
                            resource: requirement.resource().clone(),
                        });
                    }
                    return Err(ResourcePreparationError::MissingVersion {
                        resource: requirement.resource().clone(),
                    });
                };
                Ok(SealedResourceGrant {
                    resource: requirement.resource().clone(),
                    version: version.clone(),
                    kind: requirement.kind(),
                    access: requirement.access(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PreparedRunResources::new(grants.into_boxed_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::plan::{
        PlanGraphRevision, PlanProjectSessionId, PlanRegistryFingerprint, PlanResourceVersion,
    };
    use std::collections::BTreeMap;

    fn basis() -> PlanCompilationBasis {
        let mut versions = BTreeMap::new();
        versions.insert(
            PlanResourceId::from_existing("database/main".into()),
            PlanResourceVersion::from_existing("v1".into()),
        );
        PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanGraphRevision::from_existing(1),
            PlanRegistryFingerprint::from_bytes([1; 32]),
            versions,
            BTreeMap::new(),
        )
    }

    #[test]
    fn preparation_rejects_duplicate_requirement_before_grant_sealing() {
        let factory = ResourceProviderFactory::new("session".into());
        let resource = PlanResourceId::from_existing("database/main".into());
        let requirements = [
            PlanResourceRequirement::new(
                resource.clone(),
                ResourceKind::DatabaseConnection,
                ResourceAccess::Shared,
                false,
            ),
            PlanResourceRequirement::new(
                resource,
                ResourceKind::DatabaseConnection,
                ResourceAccess::Shared,
                false,
            ),
        ];
        assert!(matches!(
            factory.prepare(&requirements, &basis()),
            Err(ResourcePreparationError::DuplicateRequirement)
        ));
    }
}
