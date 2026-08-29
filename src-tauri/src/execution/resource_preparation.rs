use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use crate::execution::finalization::{
    CandidateEffectProjection, SealedCandidateGrant, SealedCandidateGrantSet,
};
use crate::execution::package_preparation::PreparedExecutionPlan;
use crate::execution::plan::{
    PlanProjectSessionId, PlanResourceId, PlanResourceObservedState, PlanResourceRequirement,
    PlanResourceVersion, ResourceAccess, ResourceKind,
};
use crate::execution::value::RuntimeValue;

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ResourcePreparationError {
    #[error("resource factory is bound to a different project session")]
    FactorySessionMismatch {
        expected: PlanProjectSessionId,
        actual: PlanProjectSessionId,
    },
    #[error("resource bindings belong to a different project session")]
    BindingSessionMismatch {
        expected: PlanProjectSessionId,
        actual: PlanProjectSessionId,
    },
    #[error("resource requirement is duplicated")]
    DuplicateRequirement,
    #[error("resource binding is duplicated")]
    DuplicateBinding,
    #[error("resource binding is not declared by the prepared request")]
    UnexpectedBinding { resource: PlanResourceId },
    #[error("resource binding does not match its declared requirement")]
    RequirementMismatch { resource: PlanResourceId },
    #[error("resource requirement has no matching binding")]
    MissingBinding { resource: PlanResourceId },
    #[error("resource requirement is missing a version observation")]
    MissingVersion { resource: PlanResourceId },
    #[error("resource requirement is missing an observation")]
    MissingObservation { resource: PlanResourceId },
    #[error("resource observation does not match the basis version")]
    ObservationVersionMismatch {
        resource: PlanResourceId,
        expected: PlanResourceVersion,
        actual: PlanResourceVersion,
    },
    #[error("resource binding version does not match the basis version")]
    VersionMismatch {
        resource: PlanResourceId,
        expected: PlanResourceVersion,
        actual: PlanResourceVersion,
    },
    #[error("resource requirement is unavailable")]
    Unavailable { resource: PlanResourceId },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunResourceBinding {
    requirement: PlanResourceRequirement,
    version: PlanResourceVersion,
    value: RuntimeValue,
}

impl RunResourceBinding {
    pub(crate) fn new(
        requirement: PlanResourceRequirement,
        version: PlanResourceVersion,
        value: RuntimeValue,
    ) -> Self {
        Self {
            requirement,
            version,
            value,
        }
    }

    pub(crate) fn resource(&self) -> &PlanResourceId {
        self.requirement.resource()
    }

    pub(crate) const fn version(&self) -> &PlanResourceVersion {
        &self.version
    }

    pub(crate) const fn requirement(&self) -> &PlanResourceRequirement {
        &self.requirement
    }

    pub(crate) const fn value(&self) -> &RuntimeValue {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunResourceBindings {
    project_session: PlanProjectSessionId,
    requirements: Box<[PlanResourceRequirement]>,
    bindings: Box<[RunResourceBinding]>,
}

impl RunResourceBindings {
    pub(crate) fn new(
        project_session: PlanProjectSessionId,
        requirements: impl IntoIterator<Item = PlanResourceRequirement>,
        bindings: impl IntoIterator<Item = RunResourceBinding>,
    ) -> Self {
        Self {
            project_session,
            requirements: requirements.into_iter().collect(),
            bindings: bindings.into_iter().collect(),
        }
    }

    pub(crate) const fn project_session(&self) -> &PlanProjectSessionId {
        &self.project_session
    }

    pub(crate) fn requirements(&self) -> &[PlanResourceRequirement] {
        &self.requirements
    }

    pub(crate) fn bindings(&self) -> &[RunResourceBinding] {
        &self.bindings
    }
}

pub(in crate::execution) struct RunResourceRequest<'a> {
    plan: &'a PreparedExecutionPlan,
    bindings: &'a RunResourceBindings,
}

impl<'a> RunResourceRequest<'a> {
    pub(in crate::execution) const fn new(
        plan: &'a PreparedExecutionPlan,
        bindings: &'a RunResourceBindings,
    ) -> Self {
        Self { plan, bindings }
    }
}

#[derive(Debug)]
struct SealedResourceGrant {
    resource: PlanResourceId,
    version: PlanResourceVersion,
    kind: ResourceKind,
    access: ResourceAccess,
    value: RuntimeValue,
}

#[derive(Debug)]
pub(in crate::execution) struct PreparedRunResources {
    compile_id: crate::execution::plan::PlanCompileId,
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
    fn new(
        compile_id: crate::execution::plan::PlanCompileId,
        grants: Box<[SealedResourceGrant]>,
    ) -> Self {
        Self {
            compile_id,
            grants,
            effect_buffer: ExecutionEffectBuffer::default(),
        }
    }

    pub(in crate::execution) fn value(&self, resource: &PlanResourceId) -> Option<&RuntimeValue> {
        self.grants
            .iter()
            .find(|grant| &grant.resource == resource)
            .map(|grant| &grant.value)
    }

    pub(in crate::execution) fn finish(
        self,
    ) -> (Box<[CandidateEffectProjection]>, SealedCandidateGrantSet) {
        let Self {
            compile_id,
            grants,
            effect_buffer,
        } = self;
        let candidate_grants = grants
            .into_vec()
            .into_iter()
            .map(|grant| {
                SealedCandidateGrant::new(
                    compile_id,
                    grant.resource,
                    grant.version,
                    grant.kind,
                    grant.access,
                )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        (
            effect_buffer.finish(),
            SealedCandidateGrantSet::new(candidate_grants),
        )
    }
}

#[derive(Clone)]
pub struct ResourceProviderFactory {
    session_identity: Box<str>,
}

impl ResourceProviderFactory {
    /// Test-only constructor for an empty factory. Production composition must
    /// use the composition-injected session identity; concrete database
    /// handles stay outside the Execution package.
    #[cfg(test)]
    pub(crate) fn new(session_identity: Box<str>) -> Self {
        Self { session_identity }
    }

    pub(crate) fn from_project_session(session_identity: Box<str>) -> Self {
        Self { session_identity }
    }

    pub(crate) fn bound_project_session(&self) -> PlanProjectSessionId {
        PlanProjectSessionId::from_existing(self.session_identity.clone())
    }

    pub(in crate::execution) fn prepare(
        &self,
        request: &RunResourceRequest<'_>,
    ) -> Result<PreparedRunResources, ResourcePreparationError> {
        let basis = request.plan.package().provenance().basis();
        let expected_session = self.bound_project_session();
        if expected_session != *basis.project_session() {
            return Err(ResourcePreparationError::FactorySessionMismatch {
                expected: expected_session,
                actual: basis.project_session().clone(),
            });
        }
        if request.bindings.project_session() != basis.project_session() {
            return Err(ResourcePreparationError::BindingSessionMismatch {
                expected: basis.project_session().clone(),
                actual: request.bindings.project_session().clone(),
            });
        }

        let mut requirements = BTreeMap::new();
        let mut available_versions = BTreeMap::new();
        for requirement in request.bindings.requirements() {
            let resource = requirement.resource().clone();
            if requirements
                .insert(resource.clone(), requirement.clone())
                .is_some()
            {
                return Err(ResourcePreparationError::DuplicateRequirement);
            }
            let Some(version) = basis.resource_versions().get(&resource) else {
                if requirement.optional() {
                    continue;
                }
                return Err(ResourcePreparationError::MissingVersion { resource });
            };
            let Some(observed) = basis.resource_observations().get(&resource) else {
                return Err(ResourcePreparationError::MissingObservation { resource });
            };
            match observed {
                PlanResourceObservedState::Present(observed_version)
                    if observed_version == version =>
                {
                    available_versions.insert(resource, version.clone());
                }
                PlanResourceObservedState::Present(observed_version) => {
                    return Err(ResourcePreparationError::ObservationVersionMismatch {
                        resource,
                        expected: version.clone(),
                        actual: observed_version.clone(),
                    });
                }
                PlanResourceObservedState::Absent(_) if requirement.optional() => {}
                PlanResourceObservedState::Absent(_) => {
                    return Err(ResourcePreparationError::Unavailable { resource });
                }
            }
        }

        let mut seen_bindings = BTreeSet::new();
        let mut grants = Vec::new();
        for binding in request.bindings.bindings() {
            let resource = binding.resource().clone();
            if !seen_bindings.insert(resource.clone()) {
                return Err(ResourcePreparationError::DuplicateBinding);
            }
            let Some(requirement) = requirements.get(&resource) else {
                return Err(ResourcePreparationError::UnexpectedBinding { resource });
            };
            if binding.requirement() != requirement {
                return Err(ResourcePreparationError::RequirementMismatch { resource });
            }
            let Some(expected_version) = available_versions.get(&resource) else {
                return Err(ResourcePreparationError::Unavailable { resource });
            };
            if binding.version() != expected_version {
                return Err(ResourcePreparationError::VersionMismatch {
                    resource,
                    expected: expected_version.clone(),
                    actual: binding.version().clone(),
                });
            }
            grants.push(SealedResourceGrant {
                resource,
                version: binding.version().clone(),
                kind: requirement.kind(),
                access: requirement.access(),
                value: binding.value().clone(),
            });
        }

        for (resource, requirement) in &requirements {
            if available_versions.contains_key(resource)
                && !seen_bindings.contains(resource)
                && !requirement.optional()
            {
                return Err(ResourcePreparationError::MissingBinding {
                    resource: resource.clone(),
                });
            }
        }

        Ok(PreparedRunResources::new(
            request.plan.package().provenance().compile_id(),
            grants.into_boxed_slice(),
        ))
    }
}

impl ExecutionEffectBuffer {
    fn finish(self) -> Box<[CandidateEffectProjection]> {
        self.effects
            .into_iter()
            .map(|effect| match effect {
                ExecutionEffect::VariableWrite { resource }
                | ExecutionEffect::PlotCandidate { resource } => {
                    CandidateEffectProjection::from_resource(resource)
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::identity::{ExecutionSessionId, RuntimeGeneration};
    use crate::execution::package_preparation::PreparedExecutionPlan;
    use crate::execution::plan::{
        CompiledExecutionPackage, CompiledFunctionBundle, CompiledParameterBundleBuilder,
        ExecutionPlan, PlanCompilationBasis, PlanCompileId, PlanGraphId, PlanGraphRevision,
        PlanProjectSessionId, PlanProvenance, PlanRegistryFingerprint, PlanResourceId,
        PlanResourceObservedState, PlanResourceRequirement, PlanResourceVersion,
        PlanSourceIdentity, ResourceAccess, ResourceKind,
    };
    use crate::execution::state::ExecutionRuntimeState;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn prepared_plan() -> PreparedExecutionPlan {
        let resource = PlanResourceId::from_existing("variables/answer".into());
        let version = PlanResourceVersion::from_existing("v1".into());
        let basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanGraphRevision::INITIAL,
            PlanRegistryFingerprint::from_bytes([3; 32]),
            BTreeMap::from([(resource, version)]),
            BTreeMap::from([(
                PlanResourceId::from_existing("variables/answer".into()),
                PlanResourceObservedState::Present(PlanResourceVersion::from_existing("v1".into())),
            )]),
        );
        let parameters = Arc::new(CompiledParameterBundleBuilder::new(basis.clone()).freeze());
        let functions = Arc::new(CompiledFunctionBundle::new(basis.clone(), Box::new([]), 0));
        let package = CompiledExecutionPackage::new(
            Arc::new(ExecutionPlan::empty()),
            functions,
            parameters,
            PlanProvenance::new(
                PlanSourceIdentity::new(
                    PlanGraphId::from_existing("events/main".into()),
                    None,
                    None,
                ),
                basis,
                PlanCompileId::from_existing(9),
            ),
        );
        ExecutionRuntimeState::new(
            ExecutionSessionId::new(uuid::Uuid::nil()),
            RuntimeGeneration::INITIAL,
        )
        .prepare_compiled_package(package, RuntimeGeneration::INITIAL)
        .expect("test package is valid")
    }

    fn request<'a>(plan: &'a PreparedExecutionPlan) -> RunResourceRequest<'a> {
        let requirement = PlanResourceRequirement::new(
            PlanResourceId::from_existing("variables/answer".into()),
            ResourceKind::Variable,
            ResourceAccess::Shared,
            false,
        );
        let bindings = Box::leak(Box::new(RunResourceBindings::new(
            PlanProjectSessionId::from_existing("session".into()),
            [requirement.clone()],
            [RunResourceBinding::new(
                requirement,
                PlanResourceVersion::from_existing("v1".into()),
                RuntimeValue::Integer(4),
            )],
        )));
        RunResourceRequest::new(plan, bindings)
    }

    #[test]
    fn prepare_seals_a_session_bound_grant_from_the_complete_basis() {
        let plan = prepared_plan();
        let factory = ResourceProviderFactory::new("session".into());
        let prepared = factory
            .prepare(&request(&plan))
            .expect("matching neutral binding must prepare");
        assert_eq!(
            prepared.value(&PlanResourceId::from_existing("variables/answer".into())),
            Some(&RuntimeValue::Integer(4))
        );
    }

    #[test]
    fn prepare_rejects_a_factory_bound_to_another_session() {
        let plan = prepared_plan();
        let factory = ResourceProviderFactory::new("other-session".into());
        assert!(matches!(
            factory.prepare(&request(&plan)),
            Err(ResourcePreparationError::FactorySessionMismatch { .. })
        ));
    }
}
