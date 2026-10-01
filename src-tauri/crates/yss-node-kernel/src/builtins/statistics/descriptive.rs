use super::common::{columns, group, text, value};
use super::{Input, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_data_contract::TabularScalar;
use yss_sci_contract::execution::{ScientificComputationError, ScientificExecutionControl};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.inequality.gini",
        vec![Input::fixed("series")],
        &[],
        1,
        gini,
    );
    install(
        builder,
        "yssbi.statistics.inequality.dagum_gini",
        vec![Input::fixed("series"), Input::fixed("groups")],
        &[],
        1,
        dagum_gini,
    );
    install(
        builder,
        "yssbi.statistics.inequality.theil",
        vec![Input::fixed("series"), Input::repeated("weights", 0..=1)],
        &["theil_form"],
        1,
        execute,
    );
}

fn scientific_control(inv: &KernelInvocation<'_>) -> ScientificExecutionControl {
    ScientificExecutionControl::from_shared(inv.control.cancellation.clone(), inv.control.deadline)
}

fn scientific_error(error: ScientificComputationError) -> KernelError {
    match error {
        ScientificComputationError::Cancelled => KernelError::Cancelled,
        ScientificComputationError::DeadlineExceeded => KernelError::DeadlineExceeded,
        ScientificComputationError::InvalidInput {
            violation: yss_sci_contract::execution::ScientificInputViolation::ShapeMismatch,
        } => KernelError::ShapeMismatch,
        ScientificComputationError::InvalidInput { .. } => KernelError::InvalidNumericInput,
        ScientificComputationError::ComputationFailed => KernelError::ScientificFailure,
    }
}

fn gini(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = columns(&[&inv.inputs[0]], inv, 0)?;
    inv.control
        .check_bytes(data[0].len().checked_mul(size_of::<f64>() * 4))?;
    let result = yss_sci_runtime::descriptive::gini(&data[0], &scientific_control(inv))
        .map_err(scientific_error)?;
    Ok(vec![value(result, inv)?])
}

fn dagum_gini(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    use super::super::series;
    let mut inputs = match (inv.inputs[0].unannotated(), inv.inputs[1].unannotated()) {
        (RuntimeValue::Series(values), RuntimeValue::Series(groups)) => {
            series::load(&[values.clone(), groups.clone()], inv)?
        }
        (RuntimeValue::List(values), RuntimeValue::List(groups)) => {
            if values.len() != groups.len() {
                return Err(KernelError::ShapeMismatch);
            }
            inv.control
                .check_bytes(values.len().checked_mul(size_of::<RuntimeValue>() * 8))?;
            vec![
                series::column(&inv.inputs[0], inv)?,
                series::column(&inv.inputs[1], inv)?,
            ]
        }
        _ => return Err(KernelError::UnalignedSeries),
    };
    let groups = inputs.pop().ok_or(KernelError::ShapeMismatch)?.values;
    let values = inputs.pop().ok_or(KernelError::ShapeMismatch)?.values;
    if values.len() != groups.len() {
        return Err(KernelError::ShapeMismatch);
    }
    // Include retained scalars, numeric workspaces, group IDs and sorting buffers together.
    let mut bytes = inv
        .control
        .check_bytes(values.len().checked_mul(size_of::<RuntimeValue>() * 8))?;
    for (index, group) in groups.iter().enumerate() {
        if index % 1024 == 0 {
            inv.check_control()?;
        }
        if let TabularScalar::String(label) = group {
            bytes = inv.control.check_bytes(
                label
                    .len()
                    .checked_mul(4)
                    .and_then(|n| bytes.checked_add(n)),
            )?;
        }
    }
    let mut numeric = inv.control.reserve(values.len())?;
    let mut group_ids = inv.control.reserve(groups.len())?;
    let mut labels = Vec::<TabularScalar>::new();
    for (i, (number, group)) in values.into_iter().zip(groups).enumerate() {
        if i % 1024 == 0 {
            inv.check_control()?;
        }
        numeric.push(super::super::numeric_input(Some(&RuntimeValue::Scalar(
            number,
        )))?);
        if matches!(group, TabularScalar::Null) {
            return Err(KernelError::InvalidNumericInput);
        }
        let index = match labels
            .iter()
            .position(|label| label.compare(&group) == Some(std::cmp::Ordering::Equal))
        {
            Some(index) => index,
            None => {
                labels.push(group);
                labels.len() - 1
            }
        };
        group_ids.push(index);
    }
    let label_bytes = labels.iter().try_fold(0usize, |bytes, label| {
        bytes.checked_add(match label {
            TabularScalar::String(label) => label.len(),
            _ => 32,
        })
    });
    inv.control.check_bytes(
        labels
            .len()
            .checked_mul(labels.len())
            .and_then(|pairs| pairs.checked_mul(8192))
            .and_then(|output| {
                label_bytes?
                    .checked_mul(labels.len() * 4)?
                    .checked_add(output)
            })
            .and_then(|output| output.checked_add(bytes)),
    )?;
    let result =
        yss_sci_runtime::descriptive::dagum_gini(&numeric, &group_ids, &scientific_control(inv))
            .map_err(scientific_error)?
            .map_groups(|group| labels[group].clone());
    Ok(vec![value(result, inv)?])
}

fn execute(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let weights = group(inv, "weights");
    match text(inv, "theil_form")? {
        "individual" if weights.is_empty() => {}
        "grouped" if weights.len() == 1 => {}
        _ => return Err(KernelError::InvalidParameter),
    }
    let mut inputs = vec![inv.inputs.first().ok_or(KernelError::InvalidNumericInput)?];
    inputs.extend(weights);
    let data = columns(&inputs, inv, 0)?;
    let report = yss_sci_runtime::descriptive::theil(
        &data[0],
        data.get(1).map(Vec::as_slice),
        &scientific_control(inv),
    )
    .map_err(scientific_error)?;
    Ok(vec![value(report, inv)?])
}
