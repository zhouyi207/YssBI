//! Observed-variable path adapters retain source alignment and full fitted observations.
use super::*;
use yss_sci_contract::path::ModerationOptions;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (id, advanced) in [
        ("yssbi.statistics.workflow.moderation", false),
        ("yssbi.statistics.workflow.moderation_advanced", true),
    ] {
        let mut inputs = vec![
            Input::fixed("y"),
            Input::fixed("x"),
            Input::fixed("moderator"),
        ];
        if advanced {
            inputs.push(Input::fixed("second_moderator"));
        }
        inputs.push(Input::repeated("covariates", 0..=usize::MAX));
        install(builder, id, inputs, &["probe_sd"], 2, move |inv| {
            moderation(inv, advanced)
        });
    }
}
fn moderation(
    inv: &KernelInvocation<'_>,
    advanced: bool,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let (data, retained) = materialize(inv)?;
    let n = data[0].values.len();
    let k = data
        .len()
        .checked_add(if advanced { 4 } else { 1 })
        .ok_or(KernelError::BudgetExceeded)?;
    inv.control.check_bytes((|| {
        n.checked_mul(k)?
            .checked_mul(256)?
            .checked_add(k.checked_mul(k)?.checked_mul(1024)?)?
            .checked_add(n.checked_mul(4 * size_of::<RuntimeValue>() * 8)?)?
            .checked_add(retained.checked_mul(3)?)?
            .checked_add(65536)
    })())?;
    let response = numeric(&data[0], false, inv)?;
    let columns = data[1..]
        .iter()
        .map(|c| numeric(c, false, inv))
        .collect::<Result<Vec<_>, _>>()?;
    let c = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result = yss_sci_runtime::path::moderation(
        &response,
        &columns,
        ModerationOptions {
            second_moderator: advanced,
            probe_sd: number(inv, "probe_sd")?,
        },
        &c,
    )
    .map_err(computation_error)?;
    regression_outputs(
        inv,
        &response,
        &result.model,
        inv.inputs[1..]
            .iter()
            .enumerate()
            .map(|(j, v)| input_label(v, format!("factor{}", j + 1)))
            .collect(),
        &result.diagnostics,
    )
}
