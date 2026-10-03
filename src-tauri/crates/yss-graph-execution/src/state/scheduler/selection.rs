//! Select demanded operations and reusable inputs over the admitted producer index.
use crate::error::OperationExecutionError;
use crate::result::StoredResultSnapshot;
use std::collections::BTreeMap;

pub(in crate::state) struct SelectedObservation {
    pub(in crate::state) source: crate::plan::PlanInputSource,
    pub(in crate::state) requester: crate::plan::PlanSourceIdentity,
}

pub(in crate::state) struct ExecutionSelection {
    pub(in crate::state) required_operations: Vec<bool>,
    pub(in crate::state) observations: Vec<SelectedObservation>,
    pub(in crate::state) cached_values: BTreeMap<usize, StoredResultSnapshot>,
}

pub(in crate::state) fn execution_producers(
    package: &crate::plan::ExecutionPlanPackage,
) -> Result<Vec<Option<usize>>, OperationExecutionError> {
    let operations = package.plan().operations();
    let value_count = operations
        .iter()
        .flat_map(|operation| operation.outputs())
        .map(|output| output.value().index() as usize)
        .max()
        .map_or(0, |max| max + 1);
    let mut producers = vec![None; value_count];
    for (operation_index, operation) in operations.iter().enumerate() {
        for output in operation.outputs() {
            let Some(producer) = producers.get_mut(output.value().index() as usize) else {
                return Err(OperationExecutionError::Failed);
            };
            if producer.replace(operation_index).is_some() {
                return Err(OperationExecutionError::Failed);
            }
        }
    }
    Ok(producers)
}

pub(in crate::state) fn select_execution(
    package: &crate::plan::ExecutionPlanPackage,
    demand: &crate::plan::PlanExecutionDemand,
    producers: &[Option<usize>],
    cached_output: impl Fn(&crate::plan::PlanOutputRef) -> Option<StoredResultSnapshot>,
) -> Result<ExecutionSelection, OperationExecutionError> {
    let operations = package.plan().operations();
    let consumed = operations
        .iter()
        .flat_map(|operation| operation.inputs())
        .filter_map(|binding| match binding.source() {
            crate::plan::PlanInputSource::Value(value) => Some(*value),
            crate::plan::PlanInputSource::Parameter(_) => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let include_defaults = matches!(demand, crate::plan::PlanExecutionDemand::Default)
        || matches!(
            demand,
            crate::plan::PlanExecutionDemand::Outputs {
                include_default_results: true,
                ..
            }
        );
    let mut selected = BTreeMap::new();
    if include_defaults {
        for operation in operations {
            for output in operation.outputs() {
                if !consumed.contains(&output.value()) {
                    selected.insert(
                        output.output().clone(),
                        (output.value(), output.contract().category),
                    );
                }
            }
        }
    }
    if let crate::plan::PlanExecutionDemand::Outputs { outputs, .. } = demand {
        for requested in outputs {
            let Some((value, category)) = operations.iter().find_map(|operation| {
                operation
                    .outputs()
                    .iter()
                    .find(|output| output.output() == requested)
                    .map(|output| (output.value(), output.contract().category))
            }) else {
                return Err(OperationExecutionError::DemandOutputUnavailable);
            };
            selected.insert(requested.clone(), (value, category));
        }
    }
    let observations = if include_defaults {
        operations
            .iter()
            .flat_map(|operation| {
                operation
                    .observation_intents()
                    .iter()
                    .map(|intent| match intent {
                        crate::plan::PlanObservationIntent::InspectInput { source } => {
                            SelectedObservation {
                                source: source.clone(),
                                requester: operation.source().clone(),
                            }
                        }
                    })
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if include_defaults && !operations.is_empty() && selected.is_empty() && observations.is_empty()
    {
        return Err(OperationExecutionError::Failed);
    }

    let mut required_operations = vec![false; operations.len()];
    let mut pending = selected
        .values()
        .map(|(value, _)| *value)
        .chain(
            observations
                .iter()
                .filter_map(|observation| match &observation.source {
                    crate::plan::PlanInputSource::Value(value) => Some(*value),
                    crate::plan::PlanInputSource::Parameter(_) => None,
                }),
        )
        .collect::<Vec<_>>();
    // Requested producers always run. A consumer of a requested producer must also
    // run even if it still has a valid cache before this run is admitted.
    let mut downstream = vec![Vec::new(); operations.len()];
    for (index, operation) in operations.iter().enumerate() {
        for binding in operation.inputs() {
            if let crate::plan::PlanInputSource::Value(value) = binding.source()
                && let Some(Some(producer)) = producers.get(value.index() as usize)
            {
                downstream[*producer].push(index);
            }
        }
    }
    let mut forced = vec![false; operations.len()];
    let mut affected = pending
        .iter()
        .filter_map(|value| producers.get(value.index() as usize).copied().flatten())
        .collect::<Vec<_>>();
    while let Some(index) = affected.pop() {
        if std::mem::replace(&mut forced[index], true) {
            continue;
        }
        affected.extend(&downstream[index]);
    }
    let mut reused = vec![false; operations.len()];
    let mut cached_values = BTreeMap::new();
    while let Some(value) = pending.pop() {
        let Some(producer) = producers
            .get(value.index() as usize)
            .and_then(|producer| *producer)
        else {
            return Err(OperationExecutionError::Failed);
        };
        if required_operations[producer] || reused[producer] {
            continue;
        }
        if !forced[producer]
            && let Some(cached) = operations[producer]
                .outputs()
                .iter()
                .map(|output| {
                    cached_output(output.output())
                        .map(|snapshot| (output.value().index() as usize, snapshot))
                })
                .collect::<Option<Vec<_>>>()
        {
            cached_values.extend(cached);
            reused[producer] = true;
            continue;
        }
        required_operations[producer] = true;
        pending.extend(operations[producer].inputs().iter().filter_map(|binding| {
            match binding.source() {
                crate::plan::PlanInputSource::Value(value) => Some(*value),
                crate::plan::PlanInputSource::Parameter(_) => None,
            }
        }));
    }

    Ok(ExecutionSelection {
        required_operations,
        observations,
        cached_values,
    })
}
