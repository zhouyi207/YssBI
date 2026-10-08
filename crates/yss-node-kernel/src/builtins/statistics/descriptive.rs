use super::common::{
    categories, columns, computation_error, group, materialize, numeric, text, value,
};
use super::{Input, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_data_contract::TabularScalar;
use yss_sci_contract::execution::ScientificExecutionControl;

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

fn gini(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = columns(&[&inv.inputs[0]], inv, 0)?;
    inv.control
        .check_bytes(data[0].len().checked_mul(size_of::<f64>() * 4))?;
    let result = yss_sci_runtime::descriptive::gini(&data[0], &scientific_control(inv))
        .map_err(computation_error)?;
    Ok(vec![value(result, inv)?])
}

fn dagum_gini(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let (inputs, bytes) = materialize(inv)?;
    let numeric = numeric(&inputs[0], false, inv)?;
    let (group_ids, labels) = categories(&inputs[1], false, inv)?;
    drop(inputs);
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
                    .checked_mul(labels.len())?
                    .checked_mul(4)?
                    .checked_add(output)
            })
            .and_then(|output| output.checked_add(bytes)),
    )?;
    let result =
        yss_sci_runtime::descriptive::dagum_gini(&numeric, &group_ids, &scientific_control(inv))
            .map_err(computation_error)?
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
    .map_err(computation_error)?;
    Ok(vec![value(report, inv)?])
}
