use super::common::{columns, group, text, value};
use super::{Input, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::execution::{ScientificComputationError, ScientificExecutionControl};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.inequality.theil",
        vec![Input::fixed("series"), Input::repeated("weights", 0..=1)],
        &["theil_form"],
        1,
        execute,
    );
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
        &ScientificExecutionControl::from_shared(
            inv.control.cancellation.clone(),
            inv.control.deadline,
        ),
    )
    .map_err(|error| match error {
        ScientificComputationError::Cancelled => KernelError::Cancelled,
        ScientificComputationError::DeadlineExceeded => KernelError::DeadlineExceeded,
        ScientificComputationError::InvalidInput { .. } => KernelError::InvalidNumericInput,
        ScientificComputationError::ComputationFailed => KernelError::ScientificFailure,
    })?;
    Ok(vec![value(report, inv)?])
}
