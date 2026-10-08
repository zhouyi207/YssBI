//! Shared value-DAG evaluation for a public graph run and private function frames.
use super::*;
use crate::kernel_invocation::parameter_value;
use crate::plan::{ExecutionPlanPackage, PlanOutputRef, ValueRef};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(super) struct DagExecution<'a> {
    pub package: &'a ExecutionPlanPackage,
    pub resources: &'a PreparedRunResources,
    pub control: &'a KernelControl,
    pub producers: &'a [Option<usize>],
    pub required: &'a [bool],
    pub boundaries: &'a BTreeSet<ValueRef>,
    pub seeded: BTreeMap<usize, RuntimeValue>,
    pub arguments: &'a BTreeMap<PlanOutputRef, RuntimeValue>,
}

impl NeutralPlanExecutor {
    pub(super) fn execute_dag(
        &self,
        execution: DagExecution<'_>,
        active: &mut BTreeSet<yss_graph_document::GraphResourcePath>,
    ) -> Result<Vec<SchedulerResult>, OperationExecutionError> {
        let DagExecution {
            package,
            resources,
            control,
            producers,
            required,
            boundaries,
            seeded,
            arguments,
        } = execution;
        let operations = package.plan().operations();
        let mut values: Vec<Option<RuntimeValue>> = vec![None; producers.len()];
        for (index, value) in seeded {
            values[index] = Some(value);
        }
        let mut remaining_dependencies = vec![0usize; operations.len()];
        let mut dependents = vec![Vec::new(); operations.len()];
        for (operation_index, operation) in operations.iter().enumerate() {
            if !required[operation_index] {
                continue;
            }
            for binding in operation.inputs() {
                let crate::plan::PlanInputSource::Value(reference) = binding.source() else {
                    continue;
                };
                if values[reference.index() as usize].is_some() {
                    continue;
                }
                let Some(producer) = producers
                    .get(reference.index() as usize)
                    .and_then(|producer| *producer)
                else {
                    return Err(OperationExecutionError::Failed);
                };
                remaining_dependencies[operation_index] = remaining_dependencies[operation_index]
                    .checked_add(1)
                    .ok_or(OperationExecutionError::Failed)?;
                dependents[producer].push(operation_index);
            }
        }
        let mut ready = remaining_dependencies
            .iter()
            .enumerate()
            .filter_map(|(operation_index, remaining)| {
                (required[operation_index] && *remaining == 0).then_some(operation_index)
            })
            .collect::<VecDeque<_>>();
        let mut completed_count = 0usize;
        let mut results = Vec::new();
        while let Some(operation_index) = ready.pop_front() {
            let operation = &operations[operation_index];
            control.check()?;
            let inputs = operation
                .inputs()
                .iter()
                .map(|binding| match binding.source() {
                    crate::plan::PlanInputSource::Value(reference) => values
                        .get(reference.index() as usize)
                        .and_then(Option::as_ref)
                        .cloned()
                        .ok_or(OperationExecutionError::Failed),
                    crate::plan::PlanInputSource::Parameter(handle) => {
                        let Some(payload) = package.parameters().entries().get(handle) else {
                            return Err(OperationExecutionError::Failed);
                        };
                        parameter_value(payload.value(), resources)
                            .map(std::borrow::Cow::into_owned)
                            .map_err(OperationExecutionError::from)
                    }
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.at_node(operation.source()))?;

            let mut output_values = match operation.specialization().implementation() {
                crate::plan::PlanNodeImplementation::Kernel(_) => crate::kernel_invocation::invoke(
                    &self.kernels,
                    &self.relations,
                    operation,
                    &inputs,
                    package.parameters(),
                    resources,
                    control,
                )
                .map_err(OperationExecutionError::from),
                crate::plan::PlanNodeImplementation::FunctionCall { target, arguments } => {
                    super::functions::invoke(
                        self,
                        package,
                        resources,
                        control,
                        super::functions::FunctionCall {
                            target,
                            arguments: super::functions::FunctionArguments::Ports {
                                operation,
                                bindings: arguments,
                                values: &inputs,
                            },
                        },
                        active,
                    )
                }
                crate::plan::PlanNodeImplementation::GroupMap { target, transform } => {
                    super::groups::invoke(
                        self,
                        package,
                        resources,
                        control,
                        super::groups::GroupCall {
                            operation,
                            target,
                            transform: *transform,
                            inputs: &inputs,
                        },
                        active,
                    )
                }
                crate::plan::PlanNodeImplementation::FunctionEntry => operation
                    .outputs()
                    .iter()
                    .map(|output| {
                        arguments
                            .get(output.output())
                            .cloned()
                            .ok_or(OperationExecutionError::Failed)
                    })
                    .collect(),
                crate::plan::PlanNodeImplementation::FunctionReturn => Ok(Vec::new()),
            }
            .map_err(|error| error.at_node(operation.source()))?;
            if output_values.len() != operation.outputs().len() {
                return Err(OperationExecutionError::Failed);
            }
            results::stabilize_outputs(
                &mut output_values,
                &operation
                    .outputs()
                    .iter()
                    .map(|output| boundaries.contains(&output.value()))
                    .collect::<Vec<_>>(),
                &self.relations,
                control,
            )
            .map_err(|error| OperationExecutionError::from(error).at_node(operation.source()))?;
            for (output, value) in operation.outputs().iter().zip(output_values) {
                let Some(slot) = values.get_mut(output.value().index() as usize) else {
                    return Err(OperationExecutionError::Failed);
                };
                if slot.replace(value.clone()).is_some() {
                    return Err(OperationExecutionError::Failed);
                }
                results.push(SchedulerResult {
                    value: StoredResult::new(value)
                        .with_evaluated_boundary(boundaries.contains(&output.value()))
                        .with_output_contract(output.contract().clone()),
                    category: output.contract().category,
                    output: output.output().clone(),
                });
            }
            completed_count = completed_count
                .checked_add(1)
                .ok_or(OperationExecutionError::Failed)?;
            for dependent in &dependents[operation_index] {
                let remaining = &mut remaining_dependencies[*dependent];
                *remaining = remaining
                    .checked_sub(1)
                    .ok_or(OperationExecutionError::Failed)?;
                if *remaining == 0 {
                    ready.push_back(*dependent);
                }
            }
        }
        if completed_count != required.iter().filter(|required| **required).count() {
            return Err(OperationExecutionError::Failed);
        }

        Ok(results)
    }
}
