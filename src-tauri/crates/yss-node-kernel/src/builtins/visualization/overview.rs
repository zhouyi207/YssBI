//! One materialization feeds the existing three distribution plot estimators.
use super::*;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.plot.statistical.family",
        vec![Input::fixed("values")],
        &["bins"],
        3,
        execute,
    );
}
fn execute(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = prepared(inv, false)?;
    let n = data[0].values.len();
    inv.control.check_bytes(
        n.checked_mul(96)
            .and_then(|v| {
                v.checked_add(
                    n.min(MAX_PLOT_POINTS)
                        .checked_mul(10 * STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?,
                )
            })
            .and_then(|v| {
                v.checked_add(
                    MAX_PLOT_BINS * 8 * STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES + 65536,
                )
            }),
    )?;
    let values = numeric(&data[0], inv)?;
    let c = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    Ok(vec![
        encode(sci::histogram(&values, integer(inv, "bins")?, &c), inv)?,
        encode(sci::ecdf(&values, &c), inv)?,
        encode(
            sci::box_violin(&labels(inv), std::slice::from_ref(&values), false, &c),
            inv,
        )?,
    ])
}
