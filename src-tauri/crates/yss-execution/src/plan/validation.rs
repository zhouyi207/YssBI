pub mod control;

use thiserror::Error;

use super::package::CompiledExecutionPackage;
use super::{ExecutionPlan, PlanGraphId};

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum PlanValidationError {
    #[error("input contract does not match the resolved specialization")]
    InputContractMismatch,
    #[error("function ABI does not match its unique addressed parameter and result slots")]
    FunctionAbiMismatch,
    #[error("output contract does not match the operation and output source")]
    OutputContractMismatch,
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
        for operation in self.operations() {
            for input in operation.inputs() {
                let specialization = operation.specialization();
                if !specialization.input_types().iter().any(|binding| {
                    binding.port() == input.port()
                        && binding.data_type() == &input.contract().expected_type
                }) || !specialization
                    .coercions()
                    .iter()
                    .filter(|coercion| coercion.port() == input.port())
                    .map(super::PlanInputCoercion::kind)
                    .eq(input.contract().coercions.iter().copied())
                {
                    return Err(PlanValidationError::InputContractMismatch);
                }
            }
            for output in operation.outputs() {
                let source = &output.contract().source;
                if source.graph() != operation.source().graph()
                    || source.node() != operation.source().node()
                    || source.graph() != output.output().graph()
                    || source.port() != Some(output.output().port())
                {
                    return Err(PlanValidationError::OutputContractMismatch);
                }
            }
        }
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

impl super::FunctionPlanAbi {
    pub(crate) fn validate(
        &self,
        plan: &ExecutionPlan,
        resource: &super::PlanResourceId,
    ) -> Result<(), PlanValidationError> {
        let mut identities = std::collections::BTreeSet::new();
        let mut addresses = std::collections::BTreeSet::new();
        for parameter in self.parameters() {
            if !identities.insert(&parameter.id)
                || !addresses.insert(&parameter.entry_output)
                || parameter.entry_output.graph().as_str() != resource.as_str()
                || !plan
                    .operations()
                    .iter()
                    .flat_map(super::PlanOperation::outputs)
                    .any(|output| {
                        output.output() == &parameter.entry_output
                            && output.contract().data_type == parameter.data_type
                    })
            {
                return Err(PlanValidationError::FunctionAbiMismatch);
            }
        }
        if self.result().is_some_and(|result| {
            !plan
                .operations()
                .iter()
                .flat_map(super::PlanOperation::inputs)
                .any(|input| {
                    input.port() == &result.return_input
                        && input.contract().expected_type == result.data_type
                })
        }) {
            return Err(PlanValidationError::FunctionAbiMismatch);
        }
        Ok(())
    }
}

impl CompiledExecutionPackage {
    pub fn validate(&self) -> Result<(), PlanValidationErrors> {
        let mut errors = Vec::new();
        if let Err(error) = self
            .plan()
            .validate_against_source_graph(self.provenance().source().graph())
        {
            errors.push(error);
        }
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
            if let Err(error) = function
                .plan()
                .validate_against_source_graph(&PlanGraphId::from_existing(
                    function.resource().as_str().into(),
                ))
                .and_then(|()| {
                    function
                        .abi()
                        .validate(function.plan(), function.resource())
                })
            {
                errors.push(error);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(PlanValidationErrors(errors.into_boxed_slice()))
        }
    }
}
