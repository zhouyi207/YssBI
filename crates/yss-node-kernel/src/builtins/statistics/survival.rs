use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_data_contract::TabularScalar;
use yss_sci_contract::{execution::*, regression::models::IterationOptions, survival::*};
use yss_sci_runtime::survival::{cox, evaluation, nonparametric, parametric};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for method in [
        "survival.kaplan_meier",
        "survival.nelson_aalen",
        "survival.logrank",
        "survival.cox",
        "survival.exponential",
        "survival.weibull",
        "survival.lognormal",
        "survival.loglogistic",
        "survival.aft",
        "survival.competing_risks",
        "survival.time_dependent_cox",
        "workflow.subgroup",
        "plot.nomogram",
        "plot.calibration",
        "plot.decision_curve",
    ] {
        let parametric = is_parametric(method);
        let cox = matches!(
            method,
            "survival.cox" | "survival.time_dependent_cox" | "workflow.subgroup"
        );
        let mut inputs = if method == "plot.nomogram" {
            vec![Input::fixed("model")]
        } else if method == "survival.time_dependent_cox" {
            vec![
                Input::fixed("start"),
                Input::fixed("stop"),
                Input::fixed("event"),
                Input::fixed("subjects"),
            ]
        } else {
            vec![
                Input::fixed("time"),
                Input::fixed(if method == "survival.competing_risks" {
                    "status"
                } else {
                    "event"
                }),
            ]
        };
        match method {
            "survival.kaplan_meier" | "survival.nelson_aalen" => {
                inputs.push(Input::repeated("groups", 0..=1))
            }
            "survival.logrank" => inputs.push(Input::fixed("groups")),
            "workflow.subgroup" => {
                inputs.extend([Input::fixed("treatment"), Input::fixed("groups")])
            }
            "plot.calibration" | "plot.decision_curve" => {
                inputs.push(Input::fixed("predicted_risk"))
            }
            _ => {}
        }
        if cox || parametric {
            inputs.push(Input::repeated(
                "x",
                if cox && method != "workflow.subgroup" {
                    1
                } else {
                    0
                }..=usize::MAX,
            ));
        }
        let mut parameters = vec![];
        if cox {
            parameters.push("survival_ties");
        }
        if cox || parametric {
            parameters.extend(["max_iterations", "tolerance"]);
        }
        if method == "survival.cox" || parametric || method.starts_with("plot.") {
            parameters.push("survival_horizon");
        }
        match method {
            "survival.aft" => parameters.push("aft_distribution"),
            "plot.nomogram" => parameters.push("nomogram_ticks"),
            "plot.calibration" => parameters.push("calibration_bins"),
            "plot.decision_curve" => parameters.extend([
                "decision_threshold_min",
                "decision_threshold_max",
                "decision_points",
            ]),
            _ => {}
        }
        install(
            builder,
            &format!("yssbi.statistics.{method}"),
            inputs,
            &parameters,
            if method == "survival.cox" || parametric {
                2
            } else {
                1
            },
            move |inv| execute(method, inv),
        );
    }
}
fn is_parametric(method: &str) -> bool {
    matches!(
        method,
        "survival.exponential"
            | "survival.weibull"
            | "survival.lognormal"
            | "survival.loglogistic"
            | "survival.aft"
    )
}
fn iteration(inv: &KernelInvocation<'_>) -> Result<IterationOptions, KernelError> {
    Ok(IterationOptions {
        max_iterations: integer(inv, "max_iterations")?,
        tolerance: number(inv, "tolerance")?,
    })
}
fn cox_options(inv: &KernelInvocation<'_>) -> Result<CoxOptions, KernelError> {
    Ok(CoxOptions {
        iteration: iteration(inv)?,
        ties: match text(inv, "survival_ties")? {
            "efron" => CoxTies::Efron,
            "breslow" => CoxTies::Breslow,
            _ => return Err(KernelError::InvalidParameter),
        },
    })
}
fn name_cox(model: &mut CoxResult, labels: &[String], skip: usize) {
    for ((coefficient, ratio), label) in model
        .coefficients
        .iter_mut()
        .zip(&mut model.hazard_ratios)
        .skip(skip)
        .zip(labels)
    {
        coefficient.term.clone_from(label);
        ratio.term.clone_from(label);
    }
}
fn status_code(v: &TabularScalar) -> Result<usize, KernelError> {
    match v {
        TabularScalar::Integer(v) => {
            usize::try_from(*v).map_err(|_| KernelError::InvalidNumericInput)
        }
        TabularScalar::Unsigned(v) => {
            usize::try_from(*v).map_err(|_| KernelError::InvalidNumericInput)
        }
        TabularScalar::Float64(v)
            if v.as_f64() >= 0.0
                && v.as_f64() < (usize::MAX as f64)
                && v.as_f64().fract() == 0.0 =>
        {
            Ok(v.as_f64() as usize)
        }
        _ => Err(KernelError::InvalidNumericInput),
    }
}
fn prediction_table(
    time: &[f64],
    event: &[f64],
    risk: &[f64],
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let fields = inv
        .outputs
        .get(1)
        .and_then(|o| o.fields.as_deref())
        .ok_or(KernelError::OutputContractMismatch)?;
    if fields.len() != 3
        || fields
            .iter()
            .zip(["time", "event", "risk"])
            .any(|(f, name)| f.name.as_ref() != name)
    {
        return Err(KernelError::OutputContractMismatch);
    }
    let columns = [
        numeric_list(time, inv)?,
        numeric_list(event, inv)?,
        numeric_list(risk, inv)?,
    ];
    // One materialized relation gives evaluation columns a common proven row domain.
    super::super::relational::materialize(fields, &columns.iter().collect::<Vec<_>>(), inv)
}
fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    if method == "plot.nomogram" {
        let model: CoxResult = decode_model(inv)?;
        let ticks = integer(inv, "nomogram_ticks")?;
        inv.control.check_bytes((|| {
            model
                .coefficients
                .len()
                .checked_add(3)?
                .checked_mul(ticks)?
                .checked_mul(384)?
                .checked_add(model.observations.checked_mul(2048)?)
        })())?;
        return Ok(vec![value(
            evaluation::nomogram(&model, number(inv, "survival_horizon")?, ticks, &control)
                .map_err(computation_error)?,
            inv,
        )?]);
    }
    let (materialized, retained) = materialize(inv)?;
    let n = materialized.first().map_or(0, |c| c.values.len());
    let index = |key: &str| {
        inv.input_keys
            .iter()
            .position(|k| *k == key)
            .ok_or(KernelError::InvalidNumericInput)
    };
    let read = |key: &str, binary| numeric(&materialized[index(key)?], binary, inv);
    let label_key = if method == "survival.time_dependent_cox" {
        "subjects"
    } else {
        "groups"
    };
    let (group_codes, group_labels) = if let Ok(i) = index(label_key) {
        categories(&materialized[i], false, inv)?
    } else {
        (vec![0; n], vec![TabularScalar::String("All".into())])
    };
    let status: Option<Vec<usize>> = if method == "survival.competing_risks" {
        Some(
            materialized[index("status")?]
                .values
                .iter()
                .map(status_code)
                .collect::<Result<Vec<_>, _>>()?,
        )
    } else {
        None
    };
    let causes = status.as_ref().map_or(0, |v| {
        v.iter()
            .filter(|&&s| s > 0)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    });
    let predictors = group(inv, "x");
    let width = predictors
        .len()
        .checked_add(if method == "workflow.subgroup" {
            group_labels.len()
        } else {
            0
        })
        .and_then(|v| v.checked_add(4))
        .ok_or(KernelError::BudgetExceeded)?;
    // Only subgroup labels become design columns; Log-rank labels size its covariance.
    let matrix_width = if method == "survival.logrank" {
        group_labels.len()
    } else {
        width
    };
    let plot_points = if method == "plot.decision_curve" {
        integer(inv, "decision_points")?
    } else {
        n
    };
    inv.control.check_bytes((|| {
        let matrix_items = matrix_width.checked_mul(matrix_width)?;
        let workspace = n
            .checked_mul(width.checked_add(8)?)?
            .checked_mul(128)?
            .checked_add(matrix_items.checked_mul(192)?)?
            .checked_add(causes.checked_mul(128)?)?;
        let output_items = n
            .checked_mul(causes.checked_add(24)?)?
            .checked_add(matrix_items)?
            .checked_add(plot_points.checked_mul(16)?)?
            .checked_add(group_labels.len().checked_mul(8)?)?;
        retained.checked_add(workspace)?.checked_add(
            output_items.checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?,
        )
    })())?;
    let labels = predictors
        .iter()
        .enumerate()
        .map(|(j, v)| input_label(v, format!("x{}", j + 1)))
        .collect::<Vec<_>>();
    let mut x = vec![];
    for (i, key) in inv.input_keys.iter().enumerate() {
        if *key == "x" {
            x.push(numeric(&materialized[i], false, inv)?);
        }
    }
    let time = read(
        if method == "survival.time_dependent_cox" {
            "stop"
        } else {
            "time"
        },
        false,
    )?;
    if let Some(status) = status {
        return Ok(vec![value(
            nonparametric::competing_risks(&time, &status, &control).map_err(computation_error)?,
            inv,
        )?]);
    }
    let event = read("event", true)?;
    let output = match method {
        "survival.kaplan_meier" | "survival.nelson_aalen" => {
            let r = nonparametric::curves(
                &time,
                &event,
                &group_codes,
                if method.ends_with("kaplan_meier") {
                    CurveMethod::KaplanMeier
                } else {
                    CurveMethod::NelsonAalen
                },
                &control,
            )
            .map_err(computation_error)?;
            value(
                CurveResult {
                    method: r.method,
                    confidence_level: r.confidence_level,
                    curves: r
                        .curves
                        .into_iter()
                        .map(|c| SurvivalCurve {
                            group: group_labels[c.group].clone(),
                            observations: c.observations,
                            events: c.events,
                            median_survival: c.median_survival,
                            points: c.points,
                        })
                        .collect(),
                },
                inv,
            )?
        }
        "survival.logrank" => {
            let r = nonparametric::logrank(&time, &event, &group_codes, &control)
                .map_err(computation_error)?;
            value(
                LogrankResult {
                    groups: r.groups.iter().map(|&g| group_labels[g].clone()).collect(),
                    observed: r.observed,
                    expected: r.expected,
                    covariance: r.covariance,
                    test: r.test,
                },
                inv,
            )?
        }
        "survival.cox" => {
            let mut r = cox::fit(&time, &event, &x, cox_options(inv)?, &control)
                .map_err(computation_error)?;
            name_cox(&mut r, &labels, 0);
            let risks = cox::event_probabilities(&r, number(inv, "survival_horizon")?)
                .map_err(computation_error)?;
            return Ok(vec![
                value(r, inv)?,
                prediction_table(&time, &event, &risks, inv)?,
            ]);
        }
        "survival.time_dependent_cox" => {
            let mut r = cox::time_dependent(
                &read("start", false)?,
                &time,
                &event,
                &group_codes,
                &x,
                cox_options(inv)?,
                &control,
            )
            .map_err(computation_error)?;
            name_cox(&mut r, &labels, 0);
            value(r, inv)?
        }
        "workflow.subgroup" => {
            let mut r = cox::subgroup(
                &time,
                &event,
                &read("treatment", true)?,
                &group_codes,
                &x,
                cox_options(inv)?,
                &control,
            )
            .map_err(computation_error)?;
            name_cox(&mut r.model, &labels, r.groups.len());
            value(
                SubgroupResult {
                    groups: r.groups.iter().map(|&g| group_labels[g].clone()).collect(),
                    group_observations: r.group_observations,
                    group_events: r.group_events,
                    treatment_hazard_ratios: r.treatment_hazard_ratios,
                    equality_test: r.equality_test,
                    model: r.model,
                },
                inv,
            )?
        }
        "plot.calibration" => value(
            evaluation::calibration(
                &time,
                &event,
                &read("predicted_risk", false)?,
                number(inv, "survival_horizon")?,
                integer(inv, "calibration_bins")?,
                &control,
            )
            .map_err(computation_error)?,
            inv,
        )?,
        "plot.decision_curve" => value(
            evaluation::decision_curve(
                &time,
                &event,
                &read("predicted_risk", false)?,
                DecisionOptions {
                    horizon: number(inv, "survival_horizon")?,
                    minimum_threshold: number(inv, "decision_threshold_min")?,
                    maximum_threshold: number(inv, "decision_threshold_max")?,
                    points: integer(inv, "decision_points")?,
                },
                &control,
            )
            .map_err(computation_error)?,
            inv,
        )?,
        _ if is_parametric(method) => {
            let distribution = match if method == "survival.aft" {
                text(inv, "aft_distribution")?
            } else {
                method.strip_prefix("survival.").unwrap()
            } {
                "exponential" => AftDistribution::Exponential,
                "weibull" => AftDistribution::Weibull,
                "lognormal" => AftDistribution::Lognormal,
                "loglogistic" => AftDistribution::Loglogistic,
                _ => return Err(KernelError::InvalidParameter),
            };
            let mut r = parametric::fit(
                &time,
                &event,
                &x,
                AftOptions {
                    distribution,
                    iteration: iteration(inv)?,
                    horizon: number(inv, "survival_horizon")?,
                },
                &control,
            )
            .map_err(computation_error)?;
            for ((c, t), label) in r
                .coefficients
                .iter_mut()
                .zip(&mut r.time_ratios)
                .skip(1)
                .zip(&labels)
            {
                c.term.clone_from(label);
                t.term.clone_from(label);
            }
            let risks = prediction_table(&time, &event, &r.event_probabilities, inv)?;
            return Ok(vec![value(r, inv)?, risks]);
        }
        _ => return Err(KernelError::InvalidParameter),
    };
    Ok(vec![output])
}
