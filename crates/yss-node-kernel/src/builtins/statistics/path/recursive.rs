use super::*;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.sem.path",
        vec![Input::repeated("variables", 2..=usize::MAX)],
        &["equations"],
        3,
        recursive,
    );
}
fn recursive(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let p = inv.inputs.len();
    inv.control
        .check_bytes(p.checked_mul(p).and_then(|v| v.checked_mul(128)))?;
    let equations = yss_sci_runtime::path::parse_equations(text(inv, "equations")?, p, &control)
        .map_err(computation_error)?;
    let (data, retained) = materialize(inv)?;
    let n = data[0].values.len();
    let k = p.checked_add(1).ok_or(KernelError::BudgetExceeded)?;
    inv.control.check_bytes((|| {
        n.checked_mul(k)?
            .checked_mul(512)?
            .checked_add(
                equations
                    .len()
                    .checked_mul(k)?
                    .checked_mul(k)?
                    .checked_mul(2048)?,
            )?
            .checked_add(
                equations
                    .len()
                    .checked_mul(n)?
                    .checked_mul(5 * size_of::<RuntimeValue>() * 8)?,
            )?
            .checked_add(
                p.checked_mul(p)?
                    .checked_mul(6 * size_of::<RuntimeValue>() * 8)?,
            )?
            .checked_add(retained.checked_mul(3)?)?
            .checked_add(65536)
    })())?;
    let x = data
        .into_iter()
        .map(|v| numeric(&v, false, inv))
        .collect::<Result<Vec<_>, _>>()?;
    let result = yss_sci_runtime::path::recursive_path(&x, &equations, &control)
        .map_err(computation_error)?;
    let report = value(
        serde_json::json!({
            "method":"observed_recursive_path_ols", "observations":n,
            "variable_names":inv.inputs.iter().enumerate().map(|(j,v)| input_label(v, format!("x{}",j+1))).collect::<Vec<_>>(),
            "equations":result.equations.iter().map(|e| serde_json::json!({
                "response":e.equation.response+1,
                "predictors":e.equation.predictors.iter().map(|p| p+1).collect::<Vec<_>>(),
                "fit":reports::equation(&e.model),
            })).collect::<Vec<_>>()
        }),
        inv,
    )?;
    let effects = numeric_table(
        &result.effects,
        1,
        [
            "source",
            "target",
            "direct",
            "indirect",
            "total",
            "standardized_total",
        ],
        |r| {
            [
                Some((r.source + 1) as f64),
                Some((r.target + 1) as f64),
                Some(r.direct),
                Some(r.indirect),
                Some(r.total),
                r.standardized_total,
            ]
        },
        inv,
    )?;
    let mut rows = inv.control.reserve(
        equations
            .len()
            .checked_mul(n)
            .ok_or(KernelError::BudgetExceeded)?,
    )?;
    for e in &result.equations {
        for (i, &response) in x[e.equation.response].iter().enumerate() {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            rows.push([
                (e.equation.response + 1) as f64,
                (i + 1) as f64,
                response,
                e.model.fitted[i],
                e.model.residuals[i],
            ]);
        }
    }
    Ok(vec![
        report,
        effects,
        numeric_table(
            &rows,
            2,
            ["variable", "observation", "response", "fitted", "residual"],
            |r| *r,
            inv,
        )?,
    ])
}
