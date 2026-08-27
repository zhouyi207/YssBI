pub mod control;

use thiserror::Error;

use super::package::CompiledExecutionPackage;

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum PlanValidationError {
    #[error("compiled function bundle basis does not match package provenance")]
    FunctionBasisMismatch,
    #[error("compiled parameter bundle basis does not match package provenance")]
    ParameterBasisMismatch,
    #[error("compiled function resource is duplicated")]
    DuplicateFunctionResource,
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("execution plan validation failed")]
pub struct PlanValidationErrors(pub Box<[PlanValidationError]>);

impl CompiledExecutionPackage {
    pub fn validate(&self) -> Result<(), PlanValidationErrors> {
        let mut errors = Vec::new();
        let basis = self.provenance().basis();
        if self.functions().basis() != basis {
            errors.push(PlanValidationError::FunctionBasisMismatch);
        }
        if self.parameters().basis() != basis {
            errors.push(PlanValidationError::ParameterBasisMismatch);
        }
        let mut resources = std::collections::BTreeSet::new();
        for function in self.functions().plans() {
            if !resources.insert(function.resource().clone()) {
                errors.push(PlanValidationError::DuplicateFunctionResource);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(PlanValidationErrors(errors.into_boxed_slice()))
        }
    }
}
