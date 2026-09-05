use std::collections::BTreeSet;
use std::sync::Arc;

use crate::identity::RuntimeGeneration;
use crate::plan::{
    CompiledExecutionPackage, CompiledFunctionPlan, CompiledParameterHandle, PlanParameterFieldId,
    PlanParameterPayload, PlanParameterValue, PlanResourceId, PlanResourceVersion,
    PlanValidationError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackagePart {
    RootPlan,
    FunctionBundle,
    ParameterBundle,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FunctionPlanPreparationError {
    #[error("compiled function resource identity is invalid")]
    InvalidResourceIdentity { resource: PlanResourceId },
    #[error("compiled function resource version identity is invalid")]
    InvalidResourceVersion {
        resource: PlanResourceId,
        version: PlanResourceVersion,
    },
    #[error("compiled function plan is invalid")]
    InvalidPlan {
        resource: PlanResourceId,
        source: PlanValidationError,
    },
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
    #[error("compiled function resource/version is duplicated")]
    DuplicateFunction {
        resource: PlanResourceId,
        version: PlanResourceVersion,
    },
    #[error("compiled function preparation failed")]
    FunctionPlan(#[source] FunctionPlanPreparationError),
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
    if package.functions().basis() != basis {
        return Err(PackagePreparationError::BasisMismatch {
            part: PackagePart::FunctionBundle,
        });
    }
    if package.parameters().basis() != basis {
        return Err(PackagePreparationError::BasisMismatch {
            part: PackagePart::ParameterBundle,
        });
    }

    validate_functions(package.functions().plans())?;
    validate_parameters(package.parameters().entries())
}

fn validate_functions(functions: &[CompiledFunctionPlan]) -> Result<(), PackagePreparationError> {
    let mut resources = BTreeSet::new();
    for function in functions {
        if function.resource().as_str().is_empty() {
            return Err(PackagePreparationError::FunctionPlan(
                FunctionPlanPreparationError::InvalidResourceIdentity {
                    resource: function.resource().clone(),
                },
            ));
        }
        if function.version().as_str().is_empty() {
            return Err(PackagePreparationError::FunctionPlan(
                FunctionPlanPreparationError::InvalidResourceVersion {
                    resource: function.resource().clone(),
                    version: function.version().clone(),
                },
            ));
        }
        if !resources.insert((function.resource().clone(), function.version().clone())) {
            return Err(PackagePreparationError::DuplicateFunction {
                resource: function.resource().clone(),
                version: function.version().clone(),
            });
        }
        function
            .plan()
            .validate_against_source_graph(&crate::plan::PlanGraphId::from_existing(
                function.resource().as_str().into(),
            ))
            .and_then(|()| {
                function
                    .abi()
                    .validate(function.plan(), function.resource())
            })
            .map_err(|source| {
                PackagePreparationError::FunctionPlan(FunctionPlanPreparationError::InvalidPlan {
                    resource: function.resource().clone(),
                    source,
                })
            })?;
    }
    Ok(())
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
        PlanParameterValue::Scalar(_) => Ok(()),
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
        CompiledExecutionPackage, CompiledFunctionBundle, CompiledFunctionPlan,
        CompiledParameterBundleBuilder, ExecutionPlan, FunctionPlanAbi, PlanCompilationBasis,
        PlanCompileId, PlanProjectSessionId, PlanProvenance, PlanRegistryFingerprint,
        PlanResourceId, PlanResourceVersion, PlanSourceIdentity,
    };
    use std::collections::BTreeMap;

    fn package() -> CompiledExecutionPackage {
        let basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanRegistryFingerprint::from_bytes([1; 32]),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let parameters = Arc::new(CompiledParameterBundleBuilder::new(basis.clone()).freeze());
        let functions = Arc::new(CompiledFunctionBundle::new(basis.clone(), Box::new([]), 8));
        CompiledExecutionPackage::new(
            Arc::new(ExecutionPlan::empty()),
            functions,
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

    #[test]
    fn rejects_duplicate_function_resource_version_before_minting() {
        let state = crate::state::ExecutionRuntimeState::new(
            ExecutionSessionId::new(uuid::Uuid::nil()),
            RuntimeGeneration::from_existing(3),
        );
        let valid = package();
        let resource = PlanResourceId::from_existing("functions/example".into());
        let version = PlanResourceVersion::from_existing("v1".into());
        let function = CompiledFunctionPlan::new(
            resource.clone(),
            version.clone(),
            Arc::new(ExecutionPlan::empty()),
            Arc::new(FunctionPlanAbi::new(Box::new([]), None)),
        );
        let duplicate = CompiledExecutionPackage::new(
            Arc::new(ExecutionPlan::empty()),
            Arc::new(CompiledFunctionBundle::new(
                valid.provenance().basis().clone(),
                Box::new([function.clone(), function]),
                8,
            )),
            Arc::clone(valid.parameters()),
            valid.provenance().clone(),
        );

        let result = state.prepare_compiled_package(duplicate, RuntimeGeneration::from_existing(3));
        match result {
            Err(PackagePreparationError::DuplicateFunction {
                resource: actual_resource,
                version: actual_version,
            }) => {
                assert_eq!(actual_resource, resource);
                assert_eq!(actual_version, version);
            }
            Err(error) => panic!("unexpected preparation error: {error:?}"),
            Ok(_) => panic!("duplicate function package was prepared"),
        }
    }
}
