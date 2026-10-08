use super::*;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.doe.range_analysis",
        vec![
            Input::fixed("y"),
            Input::repeated("factors", 1..=usize::MAX),
        ],
        &["maximize"],
        2,
        execute,
    );
}
fn execute(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let (data, retained) = materialize(inv)?;
    let response = numeric(&data[0], false, inv)?;
    let mut factors = Vec::new();
    let mut labels = Vec::new();
    for column in &data[1..] {
        let (codes, levels) = categories(column, false, inv)?;
        factors.push(codes);
        labels.push(levels);
    }
    let levels = labels
        .iter()
        .try_fold(0usize, |s, l| s.checked_add(l.len()))
        .ok_or(KernelError::BudgetExceeded)?;
    inv.control.check_bytes((|| {
        retained
            .checked_mul(3)?
            .checked_add(
                response
                    .len()
                    .checked_mul(data.len().checked_mul(32)?.checked_add(144)?)?,
            )?
            .checked_add(levels.checked_mul(
                12 * STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES
                    + 5 * (size_of::<RuntimeValue>() * 3 + 16),
            )?)?
            .checked_add(65536)
    })())?;
    drop(data);
    let c = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result =
        yss_sci_runtime::doe::range_analysis(&response, &factors, boolean(inv, "maximize")?, &c)
            .map_err(computation_error)?;
    #[derive(serde::Serialize)]
    struct Report<'a> {
        #[serde(flatten)]
        summary: &'a yss_sci_contract::doe::RangeAnalysisSummary,
        factor_names: Vec<String>,
        level_labels: Vec<Vec<yss_data_contract::TabularScalar>>,
    }
    Ok(vec![
        value(
            Report {
                summary: &result.summary,
                factor_names: group(inv, "factors")
                    .iter()
                    .enumerate()
                    .map(|(j, v)| input_label(v, format!("factor{}", j + 1)))
                    .collect(),
                level_labels: labels,
            },
            inv,
        )?,
        numeric_table(
            &result.rows,
            1,
            ["factor", "level", "observations", "total", "mean"],
            |r| {
                [
                    r.factor as f64,
                    r.level as f64,
                    r.observations as f64,
                    r.total,
                    r.mean,
                ]
            },
            inv,
        )?,
    ])
}
