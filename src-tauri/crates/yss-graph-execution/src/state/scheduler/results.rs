//! Explicit result boundaries evaluate handles; internal DAG expressions remain composable.
use std::collections::BTreeSet;
use std::sync::Arc;
use yss_node_kernel::{KernelControl, KernelError, RuntimeValue, kernel_error};
use yss_relational_contract::{RelationControl, RelationFactory};

pub(super) fn stabilize_outputs(
    values: &mut [RuntimeValue],
    boundaries: &[bool],
    relations: &Arc<dyn RelationFactory>,
    control: &KernelControl,
) -> Result<(), KernelError> {
    let control = RelationControl {
        cancellation: Arc::clone(&control.cancellation),
        deadline: control.deadline,
        max_input_bytes: control.max_input_bytes,
    };
    let mut done = vec![false; values.len()];
    for index in 0..values.len() {
        if done[index] || !boundaries[index] {
            continue;
        }
        if let RuntimeValue::Series(first) = &values[index] {
            // Freeze columns from one row domain together so Decompose retains
            // alignment across its outputs without evaluating the source per column.
            let mut names = BTreeSet::new();
            let group = values
                .iter()
                .enumerate()
                .filter_map(|(i, value)| {
                    if !boundaries[i] || done[i] {
                        return None;
                    }
                    let RuntimeValue::Series(series) = value else {
                        return None;
                    };
                    (series.relation().shares_row_domain(first.relation())
                        && names.insert(series.column().to_owned()))
                    .then_some((i, series.clone()))
                })
                .collect::<Vec<_>>();
            let columns = group
                .iter()
                .map(|(_, series)| series.clone())
                .collect::<Vec<_>>();
            let projected = first
                .relation()
                .project_series(&columns)
                .map_err(kernel_error)?;
            let stable = Arc::clone(relations)
                .snapshot(&projected, &control)
                .map_err(kernel_error)?;
            for (i, series) in group {
                values[i] = RuntimeValue::Series(
                    stable
                        .select_series(series.column())
                        .map_err(kernel_error)?,
                );
                done[i] = true;
            }
        } else {
            values[index] = stabilize(&values[index], relations, &control)?;
            done[index] = true;
        }
    }
    Ok(())
}

fn stabilize(
    value: &RuntimeValue,
    relations: &Arc<dyn RelationFactory>,
    control: &RelationControl,
) -> Result<RuntimeValue, KernelError> {
    control.check().map_err(kernel_error)?;
    if value.is_immediate() {
        return Ok(value.clone());
    }
    Ok(match value {
        RuntimeValue::Grouped(groups) => RuntimeValue::Grouped(Arc::new(
            groups
                .with_source(
                    Arc::clone(relations)
                        .snapshot(groups.source(), control)
                        .map_err(kernel_error)?,
                )
                .map_err(kernel_error)?,
        )),
        RuntimeValue::Relation(relation) => RuntimeValue::Relation(
            Arc::clone(relations)
                .snapshot(relation, control)
                .map_err(kernel_error)?,
        ),
        RuntimeValue::Series(series) => {
            let stable = Arc::clone(relations)
                .snapshot(&series.as_relation().map_err(kernel_error)?, control)
                .map_err(kernel_error)?;
            RuntimeValue::Series(
                stable
                    .select_series(series.column())
                    .map_err(kernel_error)?,
            )
        }
        RuntimeValue::List(values) => RuntimeValue::List(
            values
                .iter()
                .map(|value| stabilize(value, relations, control))
                .collect::<Result<_, _>>()?,
        ),
        RuntimeValue::Record(values) => RuntimeValue::Record(Arc::new(
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), stabilize(value, relations, control)?)))
                .collect::<Result<_, KernelError>>()?,
        )),
        _ => value.clone(),
    })
}
