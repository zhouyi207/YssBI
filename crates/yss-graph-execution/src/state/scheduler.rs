//! Execute a selected data DAG through the frozen kernel registry.
mod dag;
mod functions;
mod groups;
mod results;
mod selection;
pub(super) use selection::{ExecutionSelection, execution_producers, select_execution};

use super::control::RunExecutionControl;
use crate::error::OperationExecutionError;
use crate::resource_preparation::PreparedRunResources;
use crate::result::StoredResult;
use std::sync::Arc;
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
        let kernel_control =
            KernelControl::new(Arc::clone(&control.cancellation), control.deadline);
        let results = self.execute_dag(
            dag::DagExecution {
                package,
                resources,
                control: &kernel_control,
                producers,
                required: &selection.required_operations,
                boundaries: &selection.boundaries,
                seeded: selection
                    .cached_values
                    .iter()
                    .map(|(index, value)| (*index, value.value().value().clone()))
                    .collect(),
                arguments: &Default::default(),
            },
            &mut Default::default(),
        )?;
        let operations = package.plan().operations();
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
