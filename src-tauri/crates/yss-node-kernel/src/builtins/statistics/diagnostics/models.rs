use super::super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_data_contract::TabularScalar;
use yss_sci_contract::{
    diagnostics::model::*, execution::*, regression::fit::RegressionFit, survival::*,
};
use yss_sci_runtime::diagnostics as sci;

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (name, inputs, parameters, outputs) in [
        (
            "collinearity",
            vec![Input::repeated("variables", 1..=usize::MAX)],
            &["constant"][..],
            1,
        ),
        (
            "harman",
            vec![Input::repeated("variables", 2..=usize::MAX)],
            &[][..],
            1,
        ),
        (
            "nri_idi",
            vec![
                Input::fixed("outcome"),
                Input::fixed("reference"),
                Input::fixed("new"),
            ],
            &["nri_mode", "risk_thresholds"][..],
            1,
        ),
        (
            "ph",
            vec![
                Input::fixed("time"),
                Input::fixed("event"),
                Input::repeated("predictors", 1..=usize::MAX),
            ],
            &[
                "survival_ties",
                "time_transform",
                "max_iterations",
                "tolerance",
            ][..],
            1,
        ),
        ("residual", vec![Input::fixed("model")], &[][..], 2),
        ("cooks_distance", vec![Input::fixed("model")], &[][..], 2),
        ("aic", vec![Input::fixed("model")], &[][..], 1),
        ("bic", vec![Input::fixed("model")], &[][..], 1),
        (
            "lr",
            vec![Input::fixed("restricted"), Input::fixed("full")],
            &[][..],
            1,
        ),
        (
            "score_lm",
            vec![Input::fixed("restricted"), Input::fixed("full")],
            &[][..],
            1,
        ),
        (
            "nested_comparison",
            vec![Input::fixed("restricted"), Input::fixed("full")],
            &[][..],
            1,
        ),
    ] {
        install(
            builder,
            &format!("yssbi.statistics.diagnostic.{name}"),
            inputs,
            parameters,
            outputs,
            move |inv| execute(name, inv),
        );
    }
}
fn error(e: ScientificComputationError) -> KernelError {
    match e {
        ScientificComputationError::Cancelled => KernelError::Cancelled,
        ScientificComputationError::DeadlineExceeded => KernelError::DeadlineExceeded,
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch,
        } => KernelError::ShapeMismatch,
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange,
        } => KernelError::InvalidParameter,
        ScientificComputationError::InvalidInput { .. } => KernelError::InvalidNumericInput,
        ScientificComputationError::ComputationFailed => KernelError::ScientificFailure,
    }
}
fn workspace(
    method: &str,
    n: usize,
    p: usize,
    retained: usize,
    encoded: bool,
    inv: &KernelInvocation<'_>,
) -> Result<(), KernelError> {
    inv.control.check_bytes((|| {
        // Compact criteria only scan borrowed fits. Charge matrix work and
        // observation tables only to the diagnostics that actually build them.
        let decomposition_size = if method == "collinearity" {
            n.min(p)
        } else {
            p
        };
        let matrices = match method {
            "aic" | "bic" => 0,
            "nri_idi" => n.checked_mul(p)?.checked_mul(size_of::<f64>())?,
            _ => n
                .checked_mul(p.checked_add(12)?)?
                .checked_mul(size_of::<f64>() * 16)?
                .checked_add(
                    decomposition_size
                        .checked_mul(decomposition_size)?
                        .checked_mul(size_of::<f64>() * 16)?,
                )?,
        };
        let encoding = if encoded {
            n.checked_mul(p.checked_add(8)?)?
                .checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?
        } else {
            0
        };
        let output = if matches!(method, "residual" | "cooks_distance") {
            n.checked_mul(8)?
                .checked_mul(size_of::<RuntimeValue>() * 4)?
        } else if method == "collinearity" {
            p.checked_add(1)?
                .checked_mul(6 * STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?
        } else {
            0
        };
        retained
            .checked_add(matrices)?
            .checked_add(encoding)?
            .checked_add(output)?
            .checked_add(64 * 1024)
    })())?;
    Ok(())
}
fn model_dimensions(v: &RuntimeValue) -> Result<(usize, usize, bool), KernelError> {
    if let RuntimeValue::LinearRegression(m) = v {
        return Ok((m.fitted.len(), m.design.len(), false));
    }
    let RuntimeValue::List(residuals) = field(v, "residuals")? else {
        return Err(KernelError::InvalidNumericInput);
    };
    let RuntimeValue::List(design) = field(v, "design")? else {
        return Err(KernelError::InvalidNumericInput);
    };
    Ok((residuals.len(), design.len(), true))
}
fn with_model<T>(
    value: &RuntimeValue,
    inv: &KernelInvocation<'_>,
    f: impl FnOnce(DiagnosticModel<'_>) -> Result<T, KernelError>,
) -> Result<T, KernelError> {
    match value {
        RuntimeValue::LinearRegression(model) => f(DiagnosticModel::Linear(model)),
        RuntimeValue::Record(_) => {
            let model: RegressionFit = decode_model_value(value, inv)?;
            f(DiagnosticModel::Binary(&model))
        }
        _ => Err(KernelError::InvalidNumericInput),
    }
}
fn observation_table(
    model: &yss_sci_contract::regression::linear::LinearRegressionResult,
    r: &InfluenceOutput,
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let fields = inv
        .outputs
        .get(1)
        .and_then(|o| o.fields.as_deref())
        .ok_or(KernelError::OutputContractMismatch)?;
    let names = [
        "observation",
        "fitted",
        "residual",
        "weighted_residual",
        "leverage",
        "standardized_residual",
        "studentized_residual",
        "cooks_distance",
    ];
    if fields.len() != names.len() || fields.iter().zip(names).any(|(f, n)| f.name.as_ref() != n) {
        return Err(KernelError::OutputContractMismatch);
    }
    let nullable = |values: &[Option<f64>]| -> Result<RuntimeValue, KernelError> {
        Ok(RuntimeValue::List(
            values
                .iter()
                .map(|v| {
                    inv.check_control()?;
                    v.map_or(Ok(TabularScalar::Null.into()), |v| {
                        RuntimeValue::float64(v).map_err(|_| KernelError::NonFiniteResult)
                    })
                })
                .collect::<Result<_, _>>()?,
        ))
    };
    let columns = [
        numeric_list(
            &(1..=model.fitted.len())
                .map(|i| i as f64)
                .collect::<Vec<_>>(),
            inv,
        )?,
        numeric_list(&model.fitted, inv)?,
        numeric_list(&model.residuals, inv)?,
        numeric_list(
            &model
                .residuals
                .iter()
                .enumerate()
                .map(|(i, u)| u * model.weights.as_ref().map_or(1.0, |w| w[i].sqrt()))
                .collect::<Vec<_>>(),
            inv,
        )?,
        numeric_list(&r.leverage, inv)?,
        nullable(&r.standardized_residuals)?,
        nullable(&r.studentized_residuals)?,
        nullable(&r.cooks_distance)?,
    ];
    crate::builtins::relational::materialize(fields, &columns.iter().collect::<Vec<_>>(), inv)
}

fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    if matches!(
        method,
        "aic" | "bic" | "residual" | "cooks_distance" | "lr" | "score_lm" | "nested_comparison"
    ) {
        let dimensions = inv
            .inputs
            .iter()
            .map(model_dimensions)
            .collect::<Result<Vec<_>, _>>()?;
        let n = dimensions
            .iter()
            .try_fold(0usize, |sum, d| sum.checked_add(d.0))
            .ok_or(KernelError::BudgetExceeded)?;
        let p = dimensions.iter().map(|d| d.1).max().unwrap_or(0);
        workspace(method, n, p, 0, dimensions.iter().any(|d| d.2), inv)?;
        let first = inv.inputs.first().ok_or(KernelError::InvalidNumericInput)?;
        if matches!(method, "residual" | "cooks_distance") {
            let RuntimeValue::LinearRegression(model) = first else {
                return Err(KernelError::InvalidNumericInput);
            };
            let result = sci::influence::influence(model, &control).map_err(error)?;
            return Ok(vec![
                value(&result.summary, inv)?,
                observation_table(model, &result, inv)?,
            ]);
        }
        let result = with_model(first, inv, |model| {
            if matches!(method, "aic" | "bic") {
                let ic = sci::comparison::information_criteria(model, &control).map_err(error)?;
                #[derive(serde::Serialize)]
                struct Criterion<'a> {
                    criterion: &'a str,
                    value: f64,
                    family: String,
                    observations: usize,
                    parameters: usize,
                    log_likelihood: f64,
                }
                value(
                    Criterion {
                        criterion: method,
                        value: if method == "aic" { ic.aic } else { ic.bic },
                        family: ic.family,
                        observations: ic.observations,
                        parameters: ic.parameters,
                        log_likelihood: ic.log_likelihood,
                    },
                    inv,
                )
            } else {
                let full = inv.inputs.get(1).ok_or(KernelError::InvalidNumericInput)?;
                with_model(full, inv, |full| {
                    let kind = match method {
                        "lr" => ComparisonMethod::LikelihoodRatio,
                        "score_lm" => ComparisonMethod::Score,
                        _ => ComparisonMethod::All,
                    };
                    value(
                        sci::comparison::compare_models(model, full, kind, &control)
                            .map_err(error)?,
                        inv,
                    )
                })
            }
        })?;
        return Ok(vec![result]);
    }
    let (data, retained) = materialize(inv)?;
    let n = data.first().map_or(0, |c| c.values.len());
    workspace(method, n, data.len(), retained, false, inv)?;
    let numbers = data
        .iter()
        .enumerate()
        .map(|(i, c)| numeric(c, matches!(inv.input_keys[i], "outcome" | "event"), inv))
        .collect::<Result<Vec<_>, _>>()?;
    let result = match method {
        "collinearity" => {
            let constant = boolean(inv, "constant")?;
            let mut result =
                sci::design::collinearity(&numbers, constant, &control).map_err(error)?;
            for (i, term) in result
                .terms
                .iter_mut()
                .skip(usize::from(constant))
                .enumerate()
            {
                term.term = input_label(&inv.inputs[i], format!("x{}", i + 1));
            }
            value(result, inv)?
        }
        "harman" => value(sci::design::harman(&numbers, &control).map_err(error)?, inv)?,
        "nri_idi" => {
            let RuntimeValue::List(thresholds) = inv
                .parameter("risk_thresholds")
                .ok_or(KernelError::InvalidParameter)?
            else {
                return Err(KernelError::InvalidParameter);
            };
            let thresholds = thresholds
                .iter()
                .map(|v| crate::builtins::numeric_input(Some(v)))
                .collect::<Result<Vec<_>, _>>()?;
            let mode = match text(inv, "nri_mode")? {
                "continuous" => ReclassificationMode::Continuous,
                "categorical" => ReclassificationMode::Categorical,
                _ => return Err(KernelError::InvalidParameter),
            };
            value(
                sci::reclassification::nri_idi(
                    &numbers[0],
                    &numbers[1],
                    &numbers[2],
                    mode,
                    &thresholds,
                    &control,
                )
                .map_err(error)?,
                inv,
            )?
        }
        "ph" => {
            let options = CoxOptions {
                ties: match text(inv, "survival_ties")? {
                    "efron" => CoxTies::Efron,
                    "breslow" => CoxTies::Breslow,
                    _ => return Err(KernelError::InvalidParameter),
                },
                iteration: yss_sci_contract::regression::models::IterationOptions {
                    max_iterations: integer(inv, "max_iterations")?,
                    tolerance: number(inv, "tolerance")?,
                },
            };
            let transform = match text(inv, "time_transform")? {
                "rank" => PhTimeTransform::Rank,
                "log" => PhTimeTransform::Log,
                "identity" => PhTimeTransform::Identity,
                _ => return Err(KernelError::InvalidParameter),
            };
            let mut result = yss_sci_runtime::survival::cox::proportional_hazards(
                &numbers[0],
                &numbers[1],
                &numbers[2..],
                options,
                transform,
                &control,
            )
            .map_err(error)?;
            for (j, (coefficient, term)) in result
                .coefficients
                .iter_mut()
                .zip(&mut result.terms)
                .enumerate()
            {
                let label = input_label(&inv.inputs[j + 2], format!("x{}", j + 1));
                coefficient.term.clone_from(&label);
                term.term = label;
            }
            value(result, inv)?
        }
        _ => return Err(KernelError::InvalidParameter),
    };
    Ok(vec![result])
}
