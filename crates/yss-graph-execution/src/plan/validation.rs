pub mod control;

use thiserror::Error;

use super::{ExecutionPlan, PlanGraphId};

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum PlanValidationError {
    #[error("input contract does not match the resolved specialization")]
    InputContractMismatch,
    #[error("output contract does not match the operation and output source")]
    OutputContractMismatch,
    #[error("reroute must forward one value of the same type without parameters or coercions")]
    InvalidReroute,
    #[error("plan operation source graph is empty")]
    EmptyOperationSourceGraph,
    #[error("plan operation source graph does not match its provenance")]
    OperationSourceGraphMismatch {
        expected: PlanGraphId,
        actual: PlanGraphId,
    },
}

impl ExecutionPlan {
    pub(crate) fn validate(&self) -> Result<(), PlanValidationError> {
        for operation in self.operations() {
            if operation.specialization().implementation()
                == &super::PlanNodeImplementation::Reroute
            {
                validate_reroute(operation)?;
            }
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

fn validate_reroute(operation: &super::PlanOperation) -> Result<(), PlanValidationError> {
    let ([input], [output]) = (operation.inputs(), operation.outputs()) else {
        return Err(PlanValidationError::InvalidReroute);
    };
    if !matches!(input.source(), super::PlanInputSource::Value(_))
        || input.contract().expected_type != output.contract().data_type
        || !operation.parameters().is_empty()
        || !operation.observation_intents().is_empty()
        || !operation.specialization().coercions().is_empty()
    {
        return Err(PlanValidationError::InvalidReroute);
    }
    Ok(())
}
