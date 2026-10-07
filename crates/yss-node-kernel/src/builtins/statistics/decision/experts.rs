use super::{columns::*, *};
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.workflow.delphi",
        vec![Input::repeated("criteria", 1..=usize::MAX)],
        &["full_score"],
        2,
        execute,
    );
}
fn execute(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = read_criteria(inv)?;
    admit(&data, data.columns.len(), 10, false, inv)?;
    let c = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result =
        yss_sci_runtime::decision::experts::delphi(&data.columns, number(inv, "full_score")?, &c)
            .map_err(computation_error)?;
    Ok(vec![
        named_report(&result.summary, &data, inv)?,
        numeric_table(
            &result.rows,
            1,
            [
                "item",
                "mean",
                "standard_deviation",
                "coefficient_of_variation",
                "q1",
                "median",
                "q3",
                "minimum",
                "maximum",
                "full_score_percent",
            ],
            |r| {
                [
                    Some(r.item as f64),
                    Some(r.mean),
                    r.standard_deviation,
                    r.coefficient_of_variation,
                    Some(r.q1),
                    Some(r.median),
                    Some(r.q3),
                    Some(r.minimum),
                    Some(r.maximum),
                    Some(r.full_score_percent),
                ]
            },
            inv,
        )?,
    ])
}
