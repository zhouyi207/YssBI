use super::*;
use yss_sci_contract::decision::fuzzy::FuzzyOperator;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.decision.fuzzy_evaluation",
        vec![
            Input::repeated("memberships", 1..=usize::MAX),
            Input::repeated("criterion_weights", 0..=1),
            Input::repeated("grade_scores", 0..=1),
        ],
        &["fuzzy_operator"],
        2,
        execute,
    );
}
fn execute(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let inputs = group(inv, "memberships");
    let columns = super::super::common::columns(&inputs, inv, 0)?;
    let n = columns[0].len();
    let p = columns.len();
    let retained = n
        .checked_mul(p)
        .and_then(|v| v.checked_mul(8))
        .ok_or(KernelError::BudgetExceeded)?;
    inv.control.check_bytes(
        retained
            .checked_mul(2)
            .and_then(|v| {
                v.checked_add(
                    n.checked_add(p)?
                        .checked_mul(12 * STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?,
                )
            })
            .and_then(|v| v.checked_add(65536)),
    )?;
    let optional = |key| -> Result<Vec<f64>, KernelError> {
        let input = group(inv, key);
        if input.is_empty() {
            Ok(vec![])
        } else {
            Ok(super::super::common::columns(&input, inv, retained)?.remove(0))
        }
    };
    let weights = optional("criterion_weights")?;
    let grade_scores = optional("grade_scores")?;
    let operator = match text(inv, "fuzzy_operator")? {
        "product_sum" => FuzzyOperator::ProductSum,
        "min_max" => FuzzyOperator::MinMax,
        "product_max" => FuzzyOperator::ProductMax,
        "min_sum" => FuzzyOperator::MinSum,
        _ => return Err(KernelError::InvalidParameter),
    };
    let c = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result =
        yss_sci_runtime::decision::fuzzy::evaluate(&columns, &weights, &grade_scores, operator, &c)
            .map_err(computation_error)?;
    #[derive(serde::Serialize)]
    struct Report<'a> {
        #[serde(flatten)]
        result: &'a yss_sci_contract::decision::fuzzy::FuzzyEvaluation,
        grade_names: Vec<String>,
    }
    Ok(vec![
        value(
            Report {
                result: &result,
                grade_names: inputs
                    .iter()
                    .enumerate()
                    .map(|(j, v)| input_label(v, format!("grade{}", j + 1)))
                    .collect(),
            },
            inv,
        )?,
        numeric_list(&result.memberships, inv)?,
    ])
}
