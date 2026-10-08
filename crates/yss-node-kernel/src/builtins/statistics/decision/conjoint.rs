use super::*;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.decision.conjoint",
        vec![
            Input::fixed("ratings"),
            Input::repeated("factors", 1..=usize::MAX),
        ],
        &[],
        2,
        execute,
    );
}
fn execute(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let (data, retained) = materialize(inv)?;
    let response = numeric(&data[0], false, inv)?;
    let mut factors = vec![];
    let mut labels = vec![];
    for column in &data[1..] {
        let (codes, levels) = categories(column, false, inv)?;
        factors.push(codes);
        labels.push(levels);
    }
    let terms = labels
        .iter()
        .try_fold(1usize, |s, l| s.checked_add(l.len().saturating_sub(1)))
        .ok_or(KernelError::BudgetExceeded)?;
    let n = response.len();
    inv.control.check_bytes((|| {
        retained
            .checked_add(n.checked_mul(terms)?.checked_mul(96)?)?
            .checked_add(terms.checked_mul(terms)?.checked_mul(128)?)?
            .checked_add(n.checked_mul(4 * (size_of::<RuntimeValue>() * 3 + 16))?)?
            .checked_add(
                terms
                    .checked_add(labels.len())?
                    .checked_mul(16 * STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?,
            )?
            .checked_add(65536)
    })())?;
    drop(data);
    let c = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result = yss_sci_runtime::decision::conjoint::fit(&response, &factors, &c)
        .map_err(computation_error)?;
    #[derive(serde::Serialize)]
    struct Report<'a> {
        #[serde(flatten)]
        summary: &'a yss_sci_contract::decision::conjoint::ConjointSummary,
        factor_names: Vec<String>,
        level_labels: &'a [Vec<yss_data_contract::TabularScalar>],
    }
    Ok(vec![
        value(
            Report {
                summary: &result.summary,
                level_labels: &labels,
                factor_names: group(inv, "factors")
                    .iter()
                    .enumerate()
                    .map(|(j, v)| input_label(v, format!("factor{}", j + 1)))
                    .collect(),
            },
            inv,
        )?,
        numeric_table(
            &result.rows,
            1,
            ["observation", "observed", "fitted", "residual"],
            |r| [r.observation as f64, r.observed, r.fitted, r.residual],
            inv,
        )?,
    ])
}
