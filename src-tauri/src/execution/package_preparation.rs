use std::sync::Arc;

use crate::execution::identity::RuntimeGeneration;
use crate::execution::plan::{CompiledExecutionPackage, PlanValidationError, PlanValidationErrors};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PackagePreparationError {
    #[error("execution runtime generation changed")]
    RuntimeGenerationChanged {
        expected: RuntimeGeneration,
        actual: RuntimeGeneration,
    },
    #[error("root execution plan is invalid")]
    RootPlanInvalid { source: PlanValidationError },
    #[error("execution package is invalid")]
    InvalidPackage,
}

#[derive(Clone)]
pub struct PreparedExecutionPlan(Arc<PreparedExecutionPlanInner>);

struct PreparedExecutionPlanInner {
    package: CompiledExecutionPackage,
    generation: RuntimeGeneration,
}

impl PreparedExecutionPlan {
    pub(crate) fn package(&self) -> &CompiledExecutionPackage {
        &self.0.package
    }

    pub(crate) fn generation(&self) -> RuntimeGeneration {
        self.0.generation
    }
}

impl crate::execution::state::ExecutionRuntimeState {
    pub(crate) fn prepare_compiled_package(
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
        package.validate().map_err(map_validation_error)?;
        Ok(PreparedExecutionPlan(Arc::new(
            PreparedExecutionPlanInner {
                package,
                generation: actual,
            },
        )))
    }
}

fn map_validation_error(error: PlanValidationErrors) -> PackagePreparationError {
    if let Some(error) = error.0.into_vec().into_iter().next() {
        PackagePreparationError::RootPlanInvalid { source: error }
    } else {
        PackagePreparationError::InvalidPackage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::identity::ExecutionSessionId;
    use crate::execution::plan::{
        CompiledExecutionPackage, CompiledFunctionBundle, CompiledParameterBundleBuilder,
        ExecutionPlan, PlanCompilationBasis, PlanCompileId, PlanGraphRevision,
        PlanProjectSessionId, PlanProvenance, PlanRegistryFingerprint, PlanSourceIdentity,
    };
    use std::collections::BTreeMap;

    fn package() -> CompiledExecutionPackage {
        let basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanGraphRevision::INITIAL,
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
                    crate::execution::plan::PlanGraphId::from_existing("graph".into()),
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
        let state = crate::execution::state::ExecutionRuntimeState::new(
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
                .is_err()
        );
    }
}
