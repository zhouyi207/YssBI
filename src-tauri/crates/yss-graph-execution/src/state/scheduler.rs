//! Execute a selected data DAG through the frozen kernel registry.
mod selection;
pub(super) use selection::{ExecutionSelection, execution_producers, select_execution};

use super::control::RunExecutionControl;
use crate::error::OperationExecutionError;
use crate::kernel_invocation::parameter_value;
use crate::resource_preparation::PreparedRunResources;
use crate::result::StoredResult;
use std::{collections::VecDeque, sync::Arc};
use yss_node_kernel::{KernelControl, KernelRegistry, RuntimeValue};

#[derive(Debug)]
pub(super) struct SchedulerOutput {
    pub(super) results: Box<[SchedulerResult]>,
    pub(super) observations: Box<[SchedulerObservation]>,
}

impl SchedulerOutput {
    pub(super) fn new(
        results: Box<[SchedulerResult]>,
        observations: Box<[SchedulerObservation]>,
    ) -> Self {
        Self {
            results,
            observations,
        }
    }
}

#[derive(Debug)]
pub(super) struct SchedulerResult {
    pub(super) value: StoredResult,
    pub(super) category: crate::plan::ResultCategory,
    pub(super) output: crate::plan::PlanOutputRef,
}

#[derive(Debug)]
pub(super) struct SchedulerObservation {
    pub(super) output: crate::plan::PlanOutputRef,
    pub(super) requester: crate::plan::PlanSourceIdentity,
}

pub(super) struct PreparedPlanExecution<'a> {
    pub(super) package: &'a crate::plan::ExecutionPlanPackage,
    pub(super) resources: &'a PreparedRunResources,
    pub(super) control: &'a RunExecutionControl,
    pub(super) producers: &'a [Option<usize>],
    pub(super) selection: ExecutionSelection,
}

pub(super) trait PreparedPlanExecutor: Send + Sync {
    fn execute(
        &self,
        execution: PreparedPlanExecution<'_>,
    ) -> Result<SchedulerOutput, OperationExecutionError>;
}

pub(super) struct NeutralPlanExecutor {
    pub(super) kernels: Arc<KernelRegistry>,
    pub(super) relations: Arc<dyn yss_relational_contract::RelationFactory>,
}

impl PreparedPlanExecutor for NeutralPlanExecutor {
    fn execute(
        &self,
        execution: PreparedPlanExecution<'_>,
    ) -> Result<SchedulerOutput, OperationExecutionError> {
        let PreparedPlanExecution {
            package,
            resources,
            control,
            producers,
            selection,
        } = execution;
        let operations = package.plan().operations();
        let mut values: Vec<Option<RuntimeValue>> = vec![None; producers.len()];
        for (index, snapshot) in &selection.cached_values {
            values[*index] = Some(snapshot.value().value().clone());
        }
        let mut remaining_dependencies = vec![0usize; operations.len()];
        let mut dependents = vec![Vec::new(); operations.len()];
        for (operation_index, operation) in operations.iter().enumerate() {
            if !selection.required_operations[operation_index] {
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
                (selection.required_operations[operation_index] && *remaining == 0)
                    .then_some(operation_index)
            })
            .collect::<VecDeque<_>>();
        let mut kernel_control =
            KernelControl::new(Arc::clone(&control.cancellation), control.deadline);
        kernel_control.max_input_bytes = control.max_input_bytes;
        let mut completed_count = 0usize;
        let mut results = Vec::new();
        while let Some(operation_index) = ready.pop_front() {
            let operation = &operations[operation_index];
            kernel_control.check()?;
            let inputs = operation
                .inputs()
                .iter()
                .map(|binding| {
                    let value = match binding.source() {
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
                    }?;
                    apply_input_coercions(value, &binding.contract().coercions)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.at_node(operation.source()))?;

            let output_values = crate::kernel_invocation::invoke(
                &self.kernels,
                &self.relations,
                operation,
                &inputs,
                package.parameters(),
                resources,
                &kernel_control,
            )
            .map_err(|error| OperationExecutionError::from(error).at_node(operation.source()))?;
            if output_values.len() != operation.outputs().len() {
                return Err(OperationExecutionError::Failed);
            }
            for (output, value) in operation.outputs().iter().zip(output_values) {
                let Some(slot) = values.get_mut(output.value().index() as usize) else {
                    return Err(OperationExecutionError::Failed);
                };
                if slot.replace(value.clone()).is_some() {
                    return Err(OperationExecutionError::Failed);
                }
                results.push(SchedulerResult {
                    value: StoredResult::new(value).with_output_contract(output.contract().clone()),
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
        if completed_count
            != selection
                .required_operations
                .iter()
                .filter(|required| **required)
                .count()
        {
            return Err(OperationExecutionError::Failed);
        }

        let observations = selection
            .observations
            .into_iter()
            .map(|selected| {
                let crate::plan::PlanInputSource::Value(value) = selected.source else {
                    return Err(OperationExecutionError::Failed);
                };
                let producer = producers
                    .get(value.index() as usize)
                    .and_then(|producer| *producer)
                    .ok_or(OperationExecutionError::Failed)?;
                let output = operations[producer]
                    .outputs()
                    .iter()
                    .find(|output| output.value() == value)
                    .map(|output| output.output().clone())
                    .ok_or(OperationExecutionError::Failed)?;
                Ok(SchedulerObservation {
                    output,
                    requester: selected.requester,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SchedulerOutput::new(
            results.into_boxed_slice(),
            observations.into_boxed_slice(),
        ))
    }
}

fn apply_input_coercions(
    mut value: RuntimeValue,
    coercions: &[crate::plan::PlanInputCoercionKind],
) -> Result<RuntimeValue, OperationExecutionError> {
    for coercion in coercions {
        value = match coercion {
            // Broadcast is a kernel-owned shape operation. Keeping the scalar
            // value here makes the coercion explicit without fabricating a
            // DataSeries length in the scheduler.
            crate::plan::PlanInputCoercionKind::BroadcastScalarToSeries => value,
        };
    }
    Ok(value)
}
