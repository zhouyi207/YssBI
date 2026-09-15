use std::sync::Arc;

use crate::identity::RuntimeGeneration;
use crate::plan::{
    CompiledExecutionPackage, CompiledParameterHandle, PlanParameterFieldId, PlanParameterPayload,
    PlanParameterValue, PlanResourceId, PlanValidationError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackagePart {
    RootPlan,
    ParameterBundle,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CompiledParameterPreparationError {
    #[error("compiled parameter handle identity is invalid")]
    InvalidHandle { handle: CompiledParameterHandle },
    #[error("compiled parameter schema identity is invalid")]
    InvalidSchema { handle: CompiledParameterHandle },
    #[error("compiled parameter resource identity is invalid")]
    InvalidResource {
        handle: CompiledParameterHandle,
        resource: PlanResourceId,
    },
    #[error("compiled parameter field identity is invalid")]
    InvalidField {
        handle: CompiledParameterHandle,
        field: PlanParameterFieldId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PackagePreparationError {
    #[error("execution kernel capabilities changed")]
    KernelCapabilitiesChanged {
        expected: crate::plan::KernelFingerprint,
        actual: crate::plan::KernelFingerprint,
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
    #[error("compiled parameter preparation failed")]
    Parameters(#[source] CompiledParameterPreparationError),
}

#[derive(Clone)]
pub struct PreparedExecutionPlan(Arc<PreparedExecutionPlanInner>);

struct PreparedExecutionPlanInner {
    package: CompiledExecutionPackage,
    generation: RuntimeGeneration,
}

impl PreparedExecutionPlan {
    pub(super) fn package(&self) -> &CompiledExecutionPackage {
        &self.0.package
    }

    pub(super) fn generation(&self) -> RuntimeGeneration {
        self.0.generation
    }
}

impl crate::state::ExecutionRuntimeState {
    pub fn prepare_compiled_package(
        &self,
        package: CompiledExecutionPackage,
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

fn validate_package(package: &CompiledExecutionPackage) -> Result<(), PackagePreparationError> {
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
    parameters: &std::collections::BTreeMap<CompiledParameterHandle, PlanParameterPayload>,
) -> Result<(), PackagePreparationError> {
    for (handle, payload) in parameters {
        if handle.as_str().is_empty() {
            return Err(PackagePreparationError::Parameters(
                CompiledParameterPreparationError::InvalidHandle {
                    handle: handle.clone(),
                },
            ));
        }
        if payload.schema().as_str().is_empty() {
            return Err(PackagePreparationError::Parameters(
                CompiledParameterPreparationError::InvalidSchema {
                    handle: handle.clone(),
                },
            ));
        }
        validate_parameter_value(handle, payload.value())?;
    }
    Ok(())
}

fn validate_parameter_value(
    handle: &CompiledParameterHandle,
    value: &PlanParameterValue,
) -> Result<(), PackagePreparationError> {
    match value {
        PlanParameterValue::Scalar(_) | PlanParameterValue::Literal(_) => Ok(()),
        PlanParameterValue::Resource(resource) => {
            if resource.as_str().is_empty() {
                Err(PackagePreparationError::Parameters(
                    CompiledParameterPreparationError::InvalidResource {
                        handle: handle.clone(),
                        resource: resource.clone(),
                    },
                ))
            } else {
                Ok(())
            }
        }
        PlanParameterValue::List(values) => values
            .iter()
            .try_for_each(|value| validate_parameter_value(handle, value)),
        PlanParameterValue::Record(fields) => {
            for (field, value) in fields {
                if field.as_str().is_empty() {
                    return Err(PackagePreparationError::Parameters(
                        CompiledParameterPreparationError::InvalidField {
                            handle: handle.clone(),
                            field: field.clone(),
                        },
                    ));
                }
                validate_parameter_value(handle, value)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::ExecutionSessionId;
    use crate::plan::{
        CompiledExecutionPackage, CompiledParameterBundleBuilder, ExecutionPlan,
        PlanCompilationBasis, PlanCompileId, PlanProjectSessionId, PlanProvenance,
        PlanRegistryFingerprint, PlanSourceIdentity,
    };
    use std::collections::BTreeMap;

    fn package() -> CompiledExecutionPackage {
        let basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanRegistryFingerprint::from_bytes([1; 32]),
            crate::kernels::KernelRegistry::default().fingerprint(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let parameters = Arc::new(CompiledParameterBundleBuilder::new(basis.clone()).freeze());
        CompiledExecutionPackage::new(
            Arc::new(ExecutionPlan::empty()),
            parameters,
            PlanProvenance::new(
                PlanSourceIdentity::new(
                    crate::plan::PlanGraphId::from_existing("graph".into()),
                    None,
                    None,
                ),
                basis,
                PlanCompileId::from_existing(1),
            ),
        )
    }

    #[test]
    fn prepared_package_remains_bound_to_runtime_generation() {
        let state = crate::state::ExecutionRuntimeState::new(
            ExecutionSessionId::new(uuid::Uuid::nil()),
            RuntimeGeneration::from_existing(3),
            crate::kernels::KernelRegistry::default().into(),
        );
        let prepared = state
            .prepare_compiled_package(package(), RuntimeGeneration::from_existing(3))
            .unwrap();
        assert_eq!(prepared.generation().get(), 3);
        assert!(
            state
                .prepare_compiled_package(package(), RuntimeGeneration::from_existing(4))
                .is_err_and(|error| matches!(
                    error,
                    PackagePreparationError::RuntimeGenerationChanged { expected, actual }
                        if expected.get() == 4 && actual.get() == 3
                ))
        );
    }
}
