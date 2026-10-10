use super::{
    Input,
    common::{boolean, categories, computation_error, integer, number, numeric, text, value},
};
use crate::{
    KernelContract, KernelError, KernelId, KernelInvocation, KernelParameterKey,
    KernelRegistryBuilder, RuntimeValue,
};
use yss_sci_contract::execution::ScientificExecutionControl;
use yss_sci_contract::regression::models::*;
use yss_sci_runtime::regression::models as sci;

mod budget;

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    let methods = &[
        "regression.robust",
        "regression.hierarchical",
        "regression.stepwise",
        "regression.curve",
        "regression.nonlinear",
        "regression.nonlinear_formula",
        "regression.ridge",
        "regression.lasso",
        "regression.pls",
        "regression.logit.multinomial",
        "regression.logit.ordinal",
        "regression.logit.firth",
        "regression.poisson",
        "regression.negative_binomial",
        "regression.zero_inflated_poisson",
        "regression.zero_inflated_negative_binomial",
        "regression.tobit",
        "regression.logit.conditional",
        "regression.deming",
        "regression.quantile",
        "workflow.regression.univariate_multivariable",
        "workflow.regression.grouped",
        "workflow.regression.baseline",
        "regression.threshold",
        "transform.rcs",
        "regression.glm",
        "regression.gamma",
        "regression.inverse_gaussian",
        "regression.cloglog",
        "regression.beta",
        "regression.fractional_response",
    ];
    for &method in methods {
        let single = matches!(
            method,
            "regression.curve" | "regression.nonlinear" | "regression.deming" | "transform.rcs"
        );
        let mut inputs = vec![
            Input::fixed("y"),
            if single {
                Input::fixed("x")
            } else {
                Input::repeated("x", 1..=usize::MAX)
            },
        ];
        if matches!(
            method,
            "regression.logit.conditional" | "workflow.regression.grouped"
        ) {
            inputs.push(Input::fixed("groups"));
        }
        if method == "regression.threshold" {
            inputs.push(Input::fixed("threshold_variable"));
        }
        if matches!(
            method,
            "regression.zero_inflated_poisson" | "regression.zero_inflated_negative_binomial"
        ) {
            inputs.push(Input::repeated("inflation_predictors", 0..=usize::MAX));
        }
        let mut keys = vec![];
        if !matches!(
            method,
            "regression.logit.ordinal"
                | "regression.logit.conditional"
                | "regression.curve"
                | "regression.nonlinear"
                | "regression.nonlinear_formula"
                | "regression.deming"
                | "regression.pls"
                | "transform.rcs"
        ) {
            keys.push("constant");
        }
        let extra: &[&str] = match method {
            "regression.robust" => &["robust_loss", "tuning", "max_iterations", "tolerance"],
            "regression.ridge" => &["lambda", "standardize"],
            "regression.lasso" => &["lambda", "standardize", "max_iterations", "tolerance"],
            "regression.pls" => &["components", "standardize"],
            "regression.curve" => &["curve_family", "degree"],
            "regression.nonlinear" => &[
                "nonlinear_family",
                "initial_values",
                "max_iterations",
                "tolerance",
            ],
            "regression.nonlinear_formula" => &[
                "formula",
                "initial_values",
                "lower_bounds",
                "upper_bounds",
                "max_iterations",
                "tolerance",
            ],
            "regression.glm" => &[
                "glm_family",
                "gaussian_link",
                "binomial_link",
                "max_iterations",
                "tolerance",
            ],
            "regression.fractional_response" => &["response_link", "max_iterations", "tolerance"],
            "regression.tobit" => &["censoring", "lower", "upper", "max_iterations", "tolerance"],
            "regression.deming" => &["variance_ratio"],
            "regression.quantile" => &["quantile", "max_iterations", "tolerance"],
            "regression.hierarchical" => &["block_sizes"],
            "regression.stepwise" => &["direction", "criterion"],
            "regression.threshold" => &["trimming", "max_candidates"],
            "transform.rcs" => &["knot_mode", "knot_count", "knots"],
            "workflow.regression.univariate_multivariable"
            | "workflow.regression.grouped"
            | "workflow.regression.baseline" => &[],
            _ => &["max_iterations", "tolerance"],
        };
        keys.extend(extra);
        let optional: &[&str] = match method {
            "regression.curve" => &["degree"],
            "regression.glm" => &["gaussian_link", "binomial_link"],
            "regression.tobit" => &["upper"],
            "transform.rcs" => &["knot_count", "knots"],
            _ => &[],
        };
        let contract = KernelContract::new(
            inputs,
            keys.iter()
                .map(|key| KernelParameterKey::new((*key).into()).expect("regression key")),
            1..=1,
        )
        .expect("regression contract")
        .with_optional_parameters(
            optional
                .iter()
                .map(|key| KernelParameterKey::new((*key).into()).expect("optional key")),
        )
        .expect("optional parameters");
        builder
            .register(
                KernelId::new(format!("yssbi.statistics.{method}").into()).expect("regression ID"),
                std::num::NonZeroU32::new(match method {
                    "regression.glm" => 9,
                    "regression.curve" => 8,
                    "regression.hierarchical" | "regression.stepwise" => 8,
                    "regression.robust"
                    | "regression.logit.firth"
                    | "regression.poisson"
                    | "regression.negative_binomial"
                    | "regression.zero_inflated_poisson"
                    | "regression.zero_inflated_negative_binomial"
                    | "regression.tobit"
                    | "regression.logit.conditional"
                    | "regression.quantile"
                    | "regression.gamma"
                    | "regression.inverse_gaussian"
                    | "regression.cloglog"
                    | "regression.beta"
                    | "regression.fractional_response" => 6,
                    "regression.threshold"
                    | "workflow.regression.baseline"
                    | "workflow.regression.univariate_multivariable"
                    | "workflow.regression.grouped"
                    | "transform.rcs" => 7,
                    "regression.logit.multinomial"
                    | "regression.logit.ordinal"
                    | "regression.ridge"
                    | "regression.lasso"
                    | "regression.pls" => 5,
                    "regression.nonlinear" | "regression.nonlinear_formula" => 6,
                    "regression.deming" => 4,
                    _ => unreachable!("registered regression method"),
                })
                .unwrap(),
                contract,
                move |inv| execute(method, inv),
            )
            .expect("unique regression model kernel");
    }
}
fn typed_columns(
    inv: &KernelInvocation<'_>,
) -> Result<Vec<super::super::series::Column>, KernelError> {
    super::super::series::columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, true)
}

fn list(inv: &KernelInvocation<'_>, key: &str) -> Result<Vec<f64>, KernelError> {
    let Some(RuntimeValue::List(v)) = inv.parameter(key).map(RuntimeValue::unannotated) else {
        return Err(KernelError::InvalidParameter);
    };
    inv.control
        .check_bytes(v.len().checked_mul(size_of::<f64>()))?;
    v.iter()
        .map(|v| super::super::numeric_input(Some(v)).map_err(|_| KernelError::InvalidParameter))
        .collect()
}
fn iteration(inv: &KernelInvocation<'_>) -> Result<IterationOptions, KernelError> {
    Ok(IterationOptions {
        max_iterations: integer(inv, "max_iterations")?,
        tolerance: number(inv, "tolerance")?,
    })
}
fn response_link(inv: &KernelInvocation<'_>, key: &str) -> Result<GlmLink, KernelError> {
    match text(inv, key)? {
        "logit" => Ok(GlmLink::Logit),
        "probit" => Ok(GlmLink::Probit),
        "cloglog" => Ok(GlmLink::Cloglog),
        _ => Err(KernelError::InvalidParameter),
    }
}

fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    inv.check_control()?;
    let data = typed_columns(inv)?;
    let predictors = inv
        .input_keys
        .iter()
        .zip(&data)
        .filter(|(key, _)| **key == "x")
        .map(|(_, c)| numeric(c, false, inv))
        .collect::<Result<Vec<_>, _>>()?;
    let inflation = inv
        .input_keys
        .iter()
        .zip(&data)
        .filter(|(key, _)| **key == "inflation_predictors")
        .map(|(_, c)| numeric(c, false, inv))
        .collect::<Result<Vec<_>, _>>()?;
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    let constant = if inv.parameter("constant").is_some() {
        boolean(inv, "constant")?
    } else {
        false
    };
    let response = &data[0];
    let categorical = matches!(
        method,
        "regression.logit.multinomial" | "regression.logit.ordinal"
    );
    let (y, labels) = if categorical {
        let (codes, labels) = categories(response, method.ends_with("ordinal"), inv)?;
        (codes.into_iter().map(|v| v as f64).collect(), labels)
    } else {
        let binary = matches!(
            method,
            "regression.logit.firth" | "regression.logit.conditional" | "regression.cloglog"
        ) || (method == "regression.glm" && text(inv, "glm_family")? == "binomial");
        (numeric(response, binary, inv)?, vec![])
    };
    let group = inv
        .input_keys
        .iter()
        .position(|k| *k == "groups")
        .map(|i| categories(&data[i], false, inv))
        .transpose()?;
    budget::check(
        method,
        inv,
        &data,
        constant,
        labels.len(),
        group.as_ref().map_or(0, |(_, labels)| labels.len()),
    )?;
    let model = match method {
        "regression.robust" => sci::robust(
            &y,
            &predictors,
            RobustOptions {
                constant,
                loss: match text(inv, "robust_loss")? {
                    "huber" => RobustLoss::Huber,
                    "tukey" => RobustLoss::Tukey,
                    _ => return Err(KernelError::InvalidParameter),
                },
                tuning: number(inv, "tuning")?,
                iteration: iteration(inv)?,
            },
            &control,
        ),
        "regression.ridge" | "regression.lasso" => sci::penalized(
            &y,
            &predictors,
            PenalizedOptions {
                constant,
                standardize: boolean(inv, "standardize")?,
                penalty: if method.ends_with("ridge") {
                    Penalty::Ridge
                } else {
                    Penalty::Lasso
                },
                lambda: number(inv, "lambda")?,
                iteration: if method.ends_with("lasso") {
                    iteration(inv)?
                } else {
                    IterationOptions::default()
                },
            },
            &control,
        ),
        "regression.pls" => sci::pls(
            &y,
            &predictors,
            integer(inv, "components")?,
            boolean(inv, "standardize")?,
            &control,
        ),
        "regression.curve" => sci::curve(
            &y,
            &predictors[0],
            match text(inv, "curve_family")? {
                "polynomial" => CurveFamily::Polynomial,
                "logarithmic" => CurveFamily::Logarithmic,
                "inverse" => CurveFamily::Inverse,
                "exponential" => CurveFamily::Exponential,
                "power" => CurveFamily::Power,
                _ => return Err(KernelError::InvalidParameter),
            },
            if inv.parameter("degree").is_some() {
                integer(inv, "degree")?
            } else {
                1
            },
            &control,
        ),
        "regression.nonlinear" => sci::nonlinear(
            &y,
            &predictors[0],
            match text(inv, "nonlinear_family")? {
                "exponential" => NonlinearFamily::Exponential,
                "logistic" => NonlinearFamily::Logistic,
                "michaelis_menten" => NonlinearFamily::MichaelisMenten,
                "gompertz" => NonlinearFamily::Gompertz,
                _ => return Err(KernelError::InvalidParameter),
            },
            &list(inv, "initial_values")?,
            iteration(inv)?,
            &control,
        ),
        "regression.nonlinear_formula" => sci::nonlinear_formula(
            &y,
            &predictors,
            text(inv, "formula")?,
            &list(inv, "initial_values")?,
            &list(inv, "lower_bounds")?,
            &list(inv, "upper_bounds")?,
            iteration(inv)?,
            &control,
        ),
        "regression.deming" => {
            sci::deming(&y, &predictors[0], number(inv, "variance_ratio")?, &control)
        }
        "regression.quantile" => sci::quantile(
            &y,
            &predictors,
            constant,
            number(inv, "quantile")?,
            iteration(inv)?,
            &control,
        ),
        "regression.logit.firth" => {
            sci::firth_logit(&y, &predictors, constant, iteration(inv)?, &control)
        }
        "workflow.regression.baseline" => sci::baseline(&y, &predictors, constant, &control),
        "regression.threshold" => {
            let index = inv
                .input_keys
                .iter()
                .position(|k| *k == "threshold_variable")
                .ok_or(KernelError::InputLayoutMismatch)?;
            sci::threshold(
                &y,
                &predictors,
                &numeric(&data[index], false, inv)?,
                constant,
                number(inv, "trimming")?,
                integer(inv, "max_candidates")?,
                &control,
            )
        }
        "transform.rcs" => {
            let knots = match text(inv, "knot_mode")? {
                "auto" => sci::automatic_spline_knots(
                    &predictors[0],
                    integer(inv, "knot_count")?,
                    &control,
                )
                .map_err(computation_error)?,
                "manual" => list(inv, "knots")?,
                _ => return Err(KernelError::InvalidParameter),
            };
            sci::restricted_cubic_spline(&y, &predictors[0], &knots, &control)
        }
        "regression.hierarchical" => {
            let sizes = list(inv, "block_sizes")?;
            let sizes = if sizes.is_empty() {
                vec![1; predictors.len()]
            } else {
                if sizes
                    .iter()
                    .any(|v| v.fract() != 0.0 || !(1.0..=predictors.len() as f64).contains(v))
                {
                    return Err(KernelError::InvalidParameter);
                }
                sizes.iter().map(|v| *v as usize).collect()
            };
            return Ok(vec![value(
                sci::hierarchical(&y, &predictors, &sizes, constant, &control)
                    .map_err(computation_error)?,
                inv,
            )?]);
        }
        "regression.stepwise" => {
            let direction = match text(inv, "direction")? {
                "forward" => SelectionDirection::Forward,
                "backward" => SelectionDirection::Backward,
                "both" => SelectionDirection::Both,
                _ => return Err(KernelError::InvalidParameter),
            };
            let criterion = match text(inv, "criterion")? {
                "aic" => SelectionCriterion::Aic,
                "bic" => SelectionCriterion::Bic,
                _ => return Err(KernelError::InvalidParameter),
            };
            return Ok(vec![value(
                sci::stepwise(&y, &predictors, constant, direction, criterion, &control)
                    .map_err(computation_error)?,
                inv,
            )?]);
        }
        "workflow.regression.univariate_multivariable" => {
            return Ok(vec![value(
                sci::univariate_multivariable(&y, &predictors, constant, &control)
                    .map_err(computation_error)?,
                inv,
            )?]);
        }
        "workflow.regression.grouped" => {
            let (codes, labels) = group.ok_or(KernelError::InvalidNumericInput)?;
            let result = sci::grouped(&y, &predictors, &codes, constant, &control)
                .map_err(computation_error)?
                .map_labels(|i| labels[i].clone());
            return Ok(vec![value(result, inv)?]);
        }
        "regression.poisson"
        | "regression.gamma"
        | "regression.inverse_gaussian"
        | "regression.cloglog"
        | "regression.glm"
        | "regression.fractional_response" => {
            let family = match method {
                "regression.poisson" => GlmFamily::Poisson,
                "regression.gamma" => GlmFamily::Gamma,
                "regression.inverse_gaussian" => GlmFamily::InverseGaussian,
                "regression.cloglog" | "regression.fractional_response" => GlmFamily::Binomial,
                _ => match text(inv, "glm_family")? {
                    "gaussian" => GlmFamily::Gaussian,
                    "binomial" => GlmFamily::Binomial,
                    "poisson" => GlmFamily::Poisson,
                    "gamma" => GlmFamily::Gamma,
                    "inverse_gaussian" => GlmFamily::InverseGaussian,
                    _ => return Err(KernelError::InvalidParameter),
                },
            };
            let link = if method == "regression.cloglog" {
                GlmLink::Cloglog
            } else if method == "regression.fractional_response" {
                response_link(inv, "response_link")?
            } else {
                match family {
                    GlmFamily::Gaussian => match text(inv, "gaussian_link")? {
                        "identity" => GlmLink::Identity,
                        "log" => GlmLink::Log,
                        _ => return Err(KernelError::InvalidParameter),
                    },
                    GlmFamily::Binomial => response_link(inv, "binomial_link")?,
                    _ => GlmLink::Log,
                }
            };
            sci::glm(
                &y,
                &predictors,
                GlmOptions {
                    constant,
                    family,
                    link,
                    fractional: method == "regression.fractional_response",
                    iteration: iteration(inv)?,
                },
                &control,
            )
        }
        _ => {
            let kind = match method {
                "regression.negative_binomial" => LikelihoodMethod::NegativeBinomial,
                "regression.zero_inflated_poisson" => LikelihoodMethod::ZeroInflatedPoisson,
                "regression.zero_inflated_negative_binomial" => {
                    LikelihoodMethod::ZeroInflatedNegativeBinomial
                }
                "regression.tobit" => LikelihoodMethod::Tobit,
                "regression.beta" => LikelihoodMethod::Beta,
                "regression.logit.multinomial" => LikelihoodMethod::MultinomialLogit,
                "regression.logit.ordinal" => LikelihoodMethod::OrdinalLogit,
                "regression.logit.conditional" => LikelihoodMethod::ConditionalLogit,
                _ => return Err(KernelError::InvalidParameter),
            };
            let lower = if kind == LikelihoodMethod::Tobit {
                number(inv, "lower")?
            } else {
                0.0
            };
            let upper = if kind == LikelihoodMethod::Tobit && text(inv, "censoring")? == "both" {
                Some(number(inv, "upper")?)
            } else {
                None
            };
            sci::likelihood(
                &y,
                &predictors,
                &inflation,
                group.as_ref().map(|g| g.0.as_slice()),
                LikelihoodOptions {
                    method: kind,
                    constant,
                    lower,
                    upper,
                    iteration: iteration(inv)?,
                },
                &control,
            )
        }
    }
    .map_err(computation_error)?;
    let model = model.map_categories(|i| labels[i].clone());
    Ok(vec![value(model, inv)?])
}
#[cfg(test)]
mod tests;
