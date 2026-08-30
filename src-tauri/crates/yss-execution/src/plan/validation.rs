pub mod control;

use thiserror::Error;

use super::package::CompiledExecutionPackage;
use super::{ExecutionPlan, PlanGraphId};

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum PlanValidationError {
    #[error("plan operation source graph is empty")]
    EmptyOperationSourceGraph,
    #[error("plan operation source graph does not match its provenance")]
    OperationSourceGraphMismatch {
        expected: PlanGraphId,
        actual: PlanGraphId,
    },
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

impl ExecutionPlan {
    pub(crate) fn validate(&self) -> Result<(), PlanValidationError> {
        self.operations()
            .iter()
            .find_map(|operation| {
                operation
                    .source()
                    .graph()
                    .as_str()
                    .is_empty()
                    .then_some(PlanValidationError::EmptyOperationSourceGraph)
            })
            .map_or(Ok(()), Err)
    }

    pub(crate) fn validate_against_source_graph(
        &self,
        expected: &PlanGraphId,
    ) -> Result<(), PlanValidationError> {
        self.validate()?;
        self.operations()
            .iter()
            .find_map(|operation| {
                let actual = operation.source().graph();
                (actual != expected).then(|| PlanValidationError::OperationSourceGraphMismatch {
                    expected: expected.clone(),
                    actual: actual.clone(),
                })
            })
            .map_or(Ok(()), Err)
    }
}

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
