use super::*;
use yss_sci_contract::{
    causal::models::*,
    execution::*,
    regression::models::{IterationOptions, RegressionCoefficient},
};
use yss_sci_runtime::causal::{designs, econometrics, treatment};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for method in [
        "econometrics.gmm",
        "causal.rdd",
        "causal.psm",
        "econometrics.heckman_two_step",
        "test.heterogeneity",
        "econometrics.sfa",
        "econometrics.sur",
        "causal.ipw",
        "causal.regression_adjustment",
        "causal.aipw",
        "causal.ate",
        "causal.att",
        "causal.synthetic_control",
    ] {
        let repeated = |key, min| Input::repeated(key, min..=usize::MAX);
        let inputs = match method {
            "causal.ate" | "causal.att" => vec![Input::fixed("effects")],
            "econometrics.sur" => vec![repeated("responses", 2), repeated("predictors", 0)],
            "causal.rdd" => vec![Input::fixed("response"), Input::fixed("running")],
            "causal.synthetic_control" => vec![Input::fixed("response"), repeated("donors", 1)],
            "econometrics.gmm" => vec![
                Input::fixed("response"),
                repeated("predictors", 1),
                repeated("instruments", 1),
            ],
            "econometrics.sfa" => vec![Input::fixed("response"), repeated("predictors", 0)],
            "econometrics.heckman_two_step" => vec![
                Input::fixed("response"),
                Input::fixed("selected"),
                repeated("predictors", 0),
                repeated("selection_predictors", 1),
            ],
            "test.heterogeneity" => vec![
                Input::fixed("response"),
                Input::fixed("treatment"),
                Input::fixed("groups"),
                repeated("predictors", 0),
            ],
            _ => vec![
                Input::fixed("response"),
                Input::fixed("treatment"),
                repeated("predictors", 0),
            ],
        };
        let parameters: &[&str] = match method {
            "econometrics.gmm" => &["constant", "gmm_steps"],
            "causal.rdd" => &["rdd_cutoff", "rdd_bandwidth", "rdd_kernel"],
            "causal.psm" => &["ps_overlap", "ps_caliper", "max_iterations", "tolerance"],
            "causal.ipw" | "causal.aipw" => &[
                "ps_overlap",
                "bootstrap_replications",
                "seed",
                "max_iterations",
                "tolerance",
            ],
            "causal.regression_adjustment" => &["bootstrap_replications", "seed"],
            "econometrics.heckman_two_step" => &[
                "bootstrap_replications",
                "seed",
                "max_iterations",
                "tolerance",
            ],
            "econometrics.sfa" => &["constant", "frontier_type", "max_iterations", "tolerance"],
            "econometrics.sur" => &["constant", "equation_predictors"],
            "causal.synthetic_control" => &["pre_periods", "max_iterations", "tolerance"],
            _ => &[],
        };
        install(
            builder,
            &format!("yssbi.statistics.{method}"),
            inputs,
            parameters,
            1,
            move |inv| execute(method, inv),
        );
    }
}

fn iteration(inv: &KernelInvocation<'_>) -> Result<IterationOptions, KernelError> {
    Ok(IterationOptions {
        max_iterations: integer(inv, "max_iterations")?,
        tolerance: number(inv, "tolerance")?,
    })
}
fn bootstrap(inv: &KernelInvocation<'_>) -> Result<BootstrapOptions, KernelError> {
    Ok(BootstrapOptions {
        replications: integer(inv, "bootstrap_replications")?,
        seed: integer(inv, "seed")? as u64,
    })
}
fn name_coefficients(
    coefficients: &mut [RegressionCoefficient],
    labels: &[String],
    constant: bool,
) {
    for (coefficient, label) in coefficients
        .iter_mut()
        .skip(usize::from(constant))
        .zip(labels)
    {
        coefficient.term.clone_from(label);
    }
}

fn equation_indices(
    inv: &KernelInvocation<'_>,
    equations: usize,
    predictors: usize,
) -> Result<Vec<Vec<usize>>, KernelError> {
    let specification = text(inv, "equation_predictors")?;
    inv.control
        .check_bytes(specification.len().checked_mul(32))?;
    if specification.trim().is_empty() {
        return Ok(vec![(0..predictors).collect(); equations]);
    }
    let result = specification
        .split(';')
        .map(|equation| {
            if equation.trim() == "-" {
                return Ok(vec![]);
            }
            let indices = equation
                .split(',')
                .map(|index| {
                    index
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|v| v.checked_sub(1))
                        .filter(|&v| v < predictors)
                        .ok_or(KernelError::InvalidParameter)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if indices
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != indices.len()
            {
                return Err(KernelError::InvalidParameter);
            }
            Ok(indices)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if result.len() != equations {
        return Err(KernelError::InvalidParameter);
    }
    Ok(result)
}

fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    if matches!(method, "causal.ate" | "causal.att") {
        #[derive(serde::Serialize)]
        struct Projection {
            estimand: &'static str,
            method: TreatmentMethod,
            observations: usize,
            target_observations: usize,
            effect: TreatmentEffect,
            inference: String,
            bootstrap_replications: usize,
        }
        let r: TreatmentResult = decode_model(inv)?;
        let ate = method == "causal.ate";
        let output = Projection {
            estimand: if ate { "ATE" } else { "ATT" },
            method: r.method,
            observations: r.observations,
            target_observations: if ate { r.observations } else { r.treated },
            effect: if ate { r.ate } else { r.att },
            inference: r.inference,
            bootstrap_replications: r.bootstrap_replications,
        };
        return Ok(vec![value(output, inv)?]);
    }
    let (materialized, retained) = materialize(inv)?;
    let n = materialized.first().map_or(0, |v| v.values.len());
    let predictors = group(inv, "predictors");
    let labels = predictors
        .iter()
        .enumerate()
        .map(|(j, v)| input_label(v, format!("x{}", j + 1)))
        .collect::<Vec<_>>();
    let group_data = if method == "test.heterogeneity" {
        Some(categories(&materialized[2], false, inv)?)
    } else {
        None
    };
    let equation_count = group(inv, "responses").len();
    let selections = if method == "econometrics.sur" {
        // Bound the index matrix before even expanding the all-predictors default.
        inv.control.check_bytes(
            equation_count
                .checked_mul(predictors.len() + 1)
                .and_then(|v| v.checked_mul(16))
                .and_then(|v| retained.checked_add(v)),
        )?;
        Some(equation_indices(inv, equation_count, predictors.len())?)
    } else {
        None
    };
    let width = if let Some(selections) = &selections {
        let intercept = usize::from(boolean(inv, "constant")?);
        selections
            .iter()
            .try_fold(0usize, |sum, indices| {
                sum.checked_add(indices.len() + intercept)
            })
            .ok_or(KernelError::BudgetExceeded)?
    } else {
        materialized
            .len()
            .checked_add(
                group_data
                    .as_ref()
                    .map_or(0, |(_, labels)| labels.len().saturating_mul(2)),
            )
            .and_then(|p| p.checked_add(4))
            .ok_or(KernelError::BudgetExceeded)?
    };
    inv.control.check_bytes((|| {
        // Designs, normal/moment systems, Hessians, bootstrap copies and output
        // coexist with aligned input. SUR scales with total equation parameters.
        let workspace = n
            .checked_mul(width.checked_add(16)?)?
            .checked_mul(128)?
            .checked_add(width.checked_mul(width)?.checked_mul(192)?)?;
        let output_items = n
            .checked_mul(equation_count.checked_mul(2)?.checked_add(16)?)?
            .checked_add(width.checked_mul(width)?.checked_mul(3)?)?;
        let output = output_items.checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?;
        retained.checked_add(workspace)?.checked_add(output)
    })())?;
    let mut columns = Vec::with_capacity(materialized.len());
    for (i, column) in materialized.iter().enumerate() {
        if (method == "test.heterogeneity" && i == 2)
            || (method == "econometrics.heckman_two_step" && i == 0)
        {
            columns.push(vec![]);
        } else {
            columns.push(numeric(column, i == 1, inv)?);
        }
    }
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    let output = match method {
        "econometrics.gmm" => {
            let constant = boolean(inv, "constant")?;
            let two_step = match text(inv, "gmm_steps")? {
                "one_step" => false,
                "two_step" => true,
                _ => return Err(KernelError::InvalidParameter),
            };
            let mut result = econometrics::gmm(
                &columns[0],
                &columns[1..1 + predictors.len()],
                &columns[1 + predictors.len()..],
                GmmOptions { constant, two_step },
                &control,
            )
            .map_err(computation_error)?;
            name_coefficients(&mut result.coefficients, &labels, constant);
            value(result, inv)?
        }
        "causal.rdd" => {
            let triangular = match text(inv, "rdd_kernel")? {
                "triangular" => true,
                "uniform" => false,
                _ => return Err(KernelError::InvalidParameter),
            };
            value(
                designs::rdd(
                    &columns[0],
                    &columns[1],
                    RddOptions {
                        cutoff: number(inv, "rdd_cutoff")?,
                        bandwidth: number(inv, "rdd_bandwidth")?,
                        triangular,
                    },
                    &control,
                )
                .map_err(computation_error)?,
                inv,
            )?
        }
        "econometrics.heckman_two_step" => {
            let response = materialized[0]
                .values
                .iter()
                .zip(&columns[1])
                .map(|(y, &selected)| {
                    if selected == 0.0 {
                        return Ok(None);
                    }
                    crate::builtins::numeric_input(Some(&RuntimeValue::Scalar(y.clone()))).map(Some)
                })
                .collect::<Result<Vec<_>, KernelError>>()?;
            let mut result = econometrics::heckman(
                &response,
                &columns[1],
                &columns[2..2 + predictors.len()],
                &columns[2 + predictors.len()..],
                HeckmanOptions {
                    iteration: iteration(inv)?,
                    bootstrap: bootstrap(inv)?,
                },
                &control,
            )
            .map_err(computation_error)?;
            name_coefficients(&mut result.outcome_coefficients, &labels, true);
            let selection_labels = group(inv, "selection_predictors")
                .iter()
                .enumerate()
                .map(|(j, v)| input_label(v, format!("z{}", j + 1)))
                .collect::<Vec<_>>();
            name_coefficients(&mut result.selection_coefficients, &selection_labels, true);
            value(result, inv)?
        }
        "econometrics.sfa" => {
            let constant = boolean(inv, "constant")?;
            let cost = match text(inv, "frontier_type")? {
                "production" => false,
                "cost" => true,
                _ => return Err(KernelError::InvalidParameter),
            };
            let mut result = econometrics::frontier(
                &columns[0],
                &columns[1..],
                FrontierOptions {
                    constant,
                    cost,
                    iteration: iteration(inv)?,
                },
                &control,
            )
            .map_err(computation_error)?;
            name_coefficients(&mut result.coefficients, &labels, constant);
            value(result, inv)?
        }
        "econometrics.sur" => {
            let constant = boolean(inv, "constant")?;
            let mut result = econometrics::sur(
                &columns[..equation_count],
                &columns[equation_count..],
                selections.as_ref().expect("SUR selection"),
                constant,
                &control,
            )
            .map_err(computation_error)?;
            for equation in &mut result.equations {
                name_coefficients(
                    &mut equation.coefficients,
                    &equation
                        .predictors
                        .iter()
                        .map(|&i| labels[i - 1].clone())
                        .collect::<Vec<_>>(),
                    constant,
                );
            }
            value(result, inv)?
        }
        "test.heterogeneity" => {
            let (groups, group_labels) = group_data.expect("group data");
            let r =
                designs::heterogeneity(&columns[0], &columns[1], &groups, &columns[3..], &control)
                    .map_err(computation_error)?;
            let mut result = HeterogeneityResult {
                observations: r.observations,
                groups: group_labels,
                group_effects: r.group_effects,
                equality_test: r.equality_test,
                coefficients: r.coefficients,
                covariance: r.covariance,
            };
            name_coefficients(&mut result.coefficients, &labels, true);
            value(result, inv)?
        }
        "causal.synthetic_control" => value(
            designs::synthetic_control(
                &columns[0],
                &columns[1..],
                SyntheticControlOptions {
                    pre_periods: integer(inv, "pre_periods")?,
                    iteration: iteration(inv)?,
                },
                &control,
            )
            .map_err(computation_error)?,
            inv,
        )?,
        _ => {
            let estimator = match method {
                "causal.psm" => TreatmentMethod::Matching,
                "causal.ipw" => TreatmentMethod::Ipw,
                "causal.regression_adjustment" => TreatmentMethod::RegressionAdjustment,
                "causal.aipw" => TreatmentMethod::Aipw,
                _ => unreachable!(),
            };
            let options = TreatmentOptions {
                method: estimator,
                overlap: if estimator == TreatmentMethod::RegressionAdjustment {
                    0.0
                } else {
                    number(inv, "ps_overlap")?
                },
                caliper: if estimator == TreatmentMethod::Matching {
                    number(inv, "ps_caliper")?
                } else {
                    0.0
                },
                iteration: if estimator == TreatmentMethod::RegressionAdjustment {
                    IterationOptions::default()
                } else {
                    iteration(inv)?
                },
                bootstrap: if estimator == TreatmentMethod::Matching {
                    BootstrapOptions {
                        replications: 0,
                        seed: 0,
                    }
                } else {
                    bootstrap(inv)?
                },
            };
            value(
                treatment::estimate(&columns[0], &columns[1], &columns[2..], options, &control)
                    .map_err(computation_error)?,
                inv,
            )?
        }
    };
    Ok(vec![output])
}
