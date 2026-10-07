use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::{execution::ScientificExecutionControl as Control, inference::*};
use yss_sci_runtime::inference as sci;

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.inference.cluster_robust",
        vec![
            Input::fixed("y"),
            Input::fixed("clusters"),
            Input::repeated("x", 1..=usize::MAX),
        ],
        &["constant"],
        1,
        cluster,
    );
    install(
        builder,
        "yssbi.statistics.postestimation.adjusted_predictions",
        vec![Input::fixed("model")],
        &["evaluation", "at", "confidence_level"],
        1,
        predictions,
    );
    install(
        builder,
        "yssbi.statistics.inference.confidence_interval",
        vec![Input::fixed("estimates"), Input::fixed("standard_errors")],
        &["confidence_level", "degrees_of_freedom"],
        2,
        intervals,
    );
    install(
        builder,
        "yssbi.statistics.posthoc.multiple_comparisons",
        vec![Input::fixed("y"), Input::fixed("groups")],
        &["confidence_level", "equal_variances", "adjustment"],
        2,
        comparisons,
    );
}
fn cluster(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let (columns, retained) = materialize(inv)?;
    let n = columns[0].values.len();
    let constant = boolean(inv, "constant")?;
    let k = columns.len() - 2 + usize::from(constant);
    inv.control.check_bytes((|| {
        let input = n.checked_mul(k.checked_add(4)?)?.checked_mul(8 * 12)?;
        let matrix = k
            .checked_mul(k)?
            .checked_mul(8 * 16 + STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?;
        retained
            .checked_add(input)?
            .checked_add(matrix)?
            .checked_add(65536)
    })())?;
    let response = numeric(&columns[0], false, inv)?;
    let (groups, _) = categories(&columns[1], false, inv)?;
    let predictors = columns[2..]
        .iter()
        .map(|c| numeric(c, false, inv))
        .collect::<Result<Vec<_>, _>>()?;
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let mut result = sci::cluster::fit(response, predictors, groups, constant, &control)
        .map_err(computation_error)?;
    for (j, c) in result
        .coefficients
        .iter_mut()
        .skip(usize::from(constant))
        .enumerate()
    {
        c.term = input_label(&inv.inputs[j + 2], format!("x{}", j + 1));
    }
    Ok(vec![value(result, inv)?])
}
fn predictions(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    use yss_sci_contract::regression::{fit::FittedRegression, postestimation::*};
    let model = inv.inputs.first().ok_or(KernelError::InvalidNumericInput)?;
    let (n, k, encoded) = model_dimensions(model)?;
    inv.control.check_bytes((|| {
        let retained = if encoded {
            n.checked_mul(k.checked_add(8)?)?
                .checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?
        } else {
            0
        };
        retained
            .checked_add(k.checked_mul(k)?.checked_mul(24)?)?
            .checked_add(65536)
    })())?;
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    with_model(model, inv, |fit| {
        let names = match fit {
            FittedRegression::Linear(m) => m
                .report
                .coefficients
                .iter()
                .map(|c| c.variable.clone())
                .collect::<Vec<_>>(),
            FittedRegression::Binary(m) => m.parameter_names.clone(),
        };
        let options = PredictionOptions {
            evaluation: match text(inv, "evaluation")? {
                "average" => Evaluation::Average,
                "at_means" => Evaluation::AtMeans,
                _ => return Err(KernelError::InvalidParameter),
            },
            at: yss_sci_runtime::hypothesis::parse_at_values(text(inv, "at")?, &names)
                .map_err(|_| KernelError::InvalidParameter)?,
            confidence_level: number(inv, "confidence_level")?,
        };
        let result = yss_sci_runtime::regression::postestimation::adjusted_predictions(
            fit, options, &control,
        )
        .map_err(computation_error)?;
        Ok(vec![value(result, inv)?])
    })
}
fn workspace(
    inputs: usize,
    rows: usize,
    width: usize,
    summary_cells: usize,
    retained: usize,
    inv: &KernelInvocation<'_>,
) -> Result<(), KernelError> {
    inv.control.check_bytes((|| {
        let numerical = inputs.checked_mul(128)?;
        let table = rows
            .checked_mul(width)?
            .checked_mul(size_of::<RuntimeValue>() * 3 + 16)?;
        let report = summary_cells.checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?;
        retained
            .checked_add(numerical)?
            .checked_add(table)?
            .checked_add(report)?
            .checked_add(65536)
    })())?;
    Ok(())
}
fn intervals(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let (columns, retained) = materialize(inv)?;
    let n = columns[0].values.len();
    workspace(n, n, 5, 16, retained, inv)?;
    let estimates = numeric(&columns[0], false, inv)?;
    let errors = numeric(&columns[1], false, inv)?;
    let df = number(inv, "degrees_of_freedom")?;
    if df < 0. {
        return Err(KernelError::InvalidParameter);
    }
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result = sci::intervals::calculate(
        &estimates,
        &errors,
        IntervalOptions {
            confidence_level: number(inv, "confidence_level")?,
            degrees_of_freedom: (df > 0.).then_some(df),
        },
        &control,
    )
    .map_err(computation_error)?;
    Ok(vec![
        value(result.summary, inv)?,
        numeric_table(
            &result.rows,
            1,
            ["index", "estimate", "standard_error", "lower", "upper"],
            |r| {
                [
                    r.index as f64,
                    r.estimate,
                    r.standard_error,
                    r.lower,
                    r.upper,
                ]
            },
            inv,
        )?,
    ])
}
fn comparisons(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let (columns, retained) = materialize(inv)?;
    let (groups, labels) = categories(&columns[1], false, inv)?;
    let pairs = labels
        .len()
        .checked_mul(labels.len().saturating_sub(1))
        .ok_or(KernelError::BudgetExceeded)?
        / 2;
    workspace(
        columns[0].values.len(),
        pairs,
        12,
        labels
            .len()
            .checked_mul(7)
            .ok_or(KernelError::BudgetExceeded)?,
        retained,
        inv,
    )?;
    let response = numeric(&columns[0], false, inv)?;
    let options = PairwiseOptions {
        confidence_level: number(inv, "confidence_level")?,
        equal_variances: boolean(inv, "equal_variances")?,
        adjustment: match text(inv, "adjustment")? {
            "holm" => ComparisonAdjustment::Holm,
            "bonferroni" => ComparisonAdjustment::Bonferroni,
            "none" => ComparisonAdjustment::None,
            _ => return Err(KernelError::InvalidParameter),
        },
    };
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result = sci::comparisons::pairwise(&response, &groups, options, &control)
        .map_err(computation_error)?;
    #[derive(serde::Serialize)]
    struct Report<'a> {
        #[serde(flatten)]
        summary: &'a PairwiseSummary,
        group_labels: &'a [yss_data_contract::TabularScalar],
    }
    let report = value(
        Report {
            summary: &result.summary,
            group_labels: &labels,
        },
        inv,
    )?;
    let label = |index: usize| -> Result<RuntimeValue, KernelError> {
        use yss_data_contract::TabularScalar;
        let scalar = labels
            .get(index.checked_sub(1).ok_or(KernelError::ShapeMismatch)?)
            .ok_or(KernelError::ShapeMismatch)?;
        let text = match scalar {
            TabularScalar::String(text) => text.to_string(),
            other => {
                serde_json::to_string(other).map_err(|_| KernelError::OutputContractMismatch)?
            }
        };
        Ok(RuntimeValue::Scalar(TabularScalar::String(text.into())))
    };
    let number = |value| RuntimeValue::float64(value).map_err(|_| KernelError::NonFiniteResult);
    let table = scalar_table(
        &result.rows,
        1,
        [
            "group_a",
            "group_b",
            "group_a_label",
            "group_b_label",
            "estimate",
            "standard_error",
            "degrees_of_freedom",
            "statistic",
            "p_value",
            "adjusted_p_value",
            "lower",
            "upper",
        ],
        |r| {
            Ok([
                number(r.group_a as f64)?,
                number(r.group_b as f64)?,
                label(r.group_a)?,
                label(r.group_b)?,
                number(r.estimate)?,
                number(r.standard_error)?,
                number(r.degrees_of_freedom)?,
                number(r.statistic)?,
                number(r.p_value)?,
                number(r.adjusted_p_value)?,
                number(r.lower)?,
                number(r.upper)?,
            ])
        },
        inv,
    )?;
    Ok(vec![report, table])
}
