use std::sync::Arc;

use crate::identity::RuntimeGeneration;
use crate::plan::{
    ExecutionPlanPackage, PlanParameterHandle, PlanParameterPayload, PlanParameterValue,
    PlanResourceId, PlanValidationError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackagePart {
    RootPlan,
    ParameterBundle,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ParameterPreparationError {
    #[error("prepared parameter handle identity is invalid")]
    InvalidHandle { handle: PlanParameterHandle },
    #[error("prepared parameter schema identity is invalid")]
    InvalidSchema { handle: PlanParameterHandle },
    #[error("prepared parameter resource identity is invalid")]
    InvalidResource {
        handle: PlanParameterHandle,
        resource: PlanResourceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PackagePreparationError {
    #[error("execution kernel capabilities changed")]
    KernelCapabilitiesChanged {
        expected: yss_node_kernel::KernelFingerprint,
        actual: yss_node_kernel::KernelFingerprint,
    },
    #[error("execution runtime generation changed")]
    RuntimeGenerationChanged {
        expected: RuntimeGeneration,
        actual: RuntimeGeneration,
    },
    #[error("root execution plan is invalid")]
    RootPlanInvalid { source: PlanValidationError },
    #[error("execution package provenance does not match its {part:?} part")]
    ProvenanceMismatch { part: PackagePart },
    #[error("execution package basis does not match its {part:?} part")]
    BasisMismatch { part: PackagePart },
    #[error("prepared parameter preparation failed")]
    Parameters(#[source] ParameterPreparationError),
}

#[derive(Clone)]
pub struct PreparedExecutionPlan(Arc<PreparedExecutionPlanInner>);

struct PreparedExecutionPlanInner {
    package: ExecutionPlanPackage,
    generation: RuntimeGeneration,
}

impl PreparedExecutionPlan {
    pub(super) fn package(&self) -> &ExecutionPlanPackage {
        &self.0.package
    }

    pub(super) fn generation(&self) -> RuntimeGeneration {
        self.0.generation
    }
}

impl crate::state::ExecutionRuntimeState {
    pub fn prepare_package(
        &self,
        package: ExecutionPlanPackage,
        expected_generation: RuntimeGeneration,
    ) -> Result<PreparedExecutionPlan, PackagePreparationError> {
        let actual = self.generation();
        if actual != expected_generation {
            return Err(PackagePreparationError::RuntimeGenerationChanged {
                expected: expected_generation,
                actual,
            });
        }
        let expected_capabilities = package.provenance().basis().kernel_fingerprint();
        let actual_capabilities = self.kernels().fingerprint();
        if expected_capabilities != actual_capabilities {
            return Err(PackagePreparationError::KernelCapabilitiesChanged {
                expected: expected_capabilities,
                actual: actual_capabilities,
            });
        }
        validate_package(&package)?;
        Ok(PreparedExecutionPlan(Arc::new(
            PreparedExecutionPlanInner {
                package,
                generation: actual,
            },
        )))
    }
}

fn validate_package(package: &ExecutionPlanPackage) -> Result<(), PackagePreparationError> {
    let provenance = package.provenance();
    let source_graph = provenance.source().graph();
    if source_graph.as_str().is_empty() {
        return Err(PackagePreparationError::ProvenanceMismatch {
            part: PackagePart::RootPlan,
        });
    }

    match package.plan().validate_against_source_graph(source_graph) {
        Ok(()) => {}
        Err(PlanValidationError::OperationSourceGraphMismatch { .. }) => {
            return Err(PackagePreparationError::ProvenanceMismatch {
                part: PackagePart::RootPlan,
            });
        }
        Err(source) => return Err(PackagePreparationError::RootPlanInvalid { source }),
    }

    let basis = provenance.basis();
    if package.parameters().basis() != basis {
        return Err(PackagePreparationError::BasisMismatch {
            part: PackagePart::ParameterBundle,
        });
    }

    validate_parameters(package.parameters().entries())
}

fn validate_parameters(
    parameters: &std::collections::BTreeMap<PlanParameterHandle, PlanParameterPayload>,
) -> Result<(), PackagePreparationError> {
    for (handle, payload) in parameters {
        if handle.as_str().is_empty() {
            return Err(PackagePreparationError::Parameters(
                ParameterPreparationError::InvalidHandle {
                    handle: handle.clone(),
                },
            ));
        }
        if payload.schema().as_str().is_empty() {
            return Err(PackagePreparationError::Parameters(
                ParameterPreparationError::InvalidSchema {
                    handle: handle.clone(),
                },
            ));
        }
        validate_parameter_value(handle, payload.value())?;
    }
    Ok(())
}

fn validate_parameter_value(
    handle: &PlanParameterHandle,
    value: &PlanParameterValue,
) -> Result<(), PackagePreparationError> {
    match value {
        PlanParameterValue::Literal(_) => Ok(()),
        PlanParameterValue::Resource(resource) => {
            if resource.as_str().is_empty() {
                Err(PackagePreparationError::Parameters(
                    ParameterPreparationError::InvalidResource {
                        handle: handle.clone(),
                        resource: resource.clone(),
                    },
                ))
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::ExecutionSessionId;
    use crate::plan::{
        ExecutionPlan, ExecutionPlanPackage, PlanBasis, PlanId, PlanParameterBundleBuilder,
        PlanProjectSessionId, PlanProvenance, PlanRegistryFingerprint, PlanSourceIdentity,
    };
    use std::collections::BTreeMap;

    fn package() -> ExecutionPlanPackage {
        let basis = PlanBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanRegistryFingerprint::from_bytes([1; 32]),
            yss_node_kernel::KernelRegistry::default().fingerprint(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let parameters = Arc::new(PlanParameterBundleBuilder::new(basis.clone()).freeze());
        ExecutionPlanPackage::new(
            Arc::new(ExecutionPlan::empty()),
            parameters,
            PlanProvenance::new(
                PlanSourceIdentity::new(
                    crate::plan::PlanGraphId::from_existing("graph".into()),
                    None,
                    None,
                ),
                basis,
                PlanId::from_existing(1),
            ),
        )
    }

    #[test]
    fn prepared_package_remains_bound_to_runtime_generation() {
        let state = crate::state::ExecutionRuntimeState::new(
            ExecutionSessionId::new(uuid::Uuid::nil()),
            RuntimeGeneration::from_existing(3),
            yss_node_kernel::KernelRegistry::default().into(),
            crate::test_relations(),
        );
        let prepared = state
            .prepare_package(package(), RuntimeGeneration::from_existing(3))
            .unwrap();
        assert_eq!(prepared.generation().get(), 3);
        assert!(
            state
                .prepare_package(package(), RuntimeGeneration::from_existing(4))
                .is_err_and(|error| matches!(
                    error,
                    PackagePreparationError::RuntimeGenerationChanged { expected, actual }
                        if expected.get() == 4 && actual.get() == 3
                ))
        );
    }
}
