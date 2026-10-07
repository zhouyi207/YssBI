use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_data_contract::TabularScalar;
use yss_sci_contract::regression::postestimation::Evaluation;
use yss_sci_contract::regression::{
    discrete::BinaryOptions,
    fit::BinaryRegressionLink,
    prais::{PraisConfig, PraisTransform},
};

#[derive(Clone, Copy)]
enum Method {
    Binary(BinaryRegressionLink),
    Prais,
}

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (name, method) in [
        ("logit", Method::Binary(BinaryRegressionLink::Logit)),
        ("probit", Method::Binary(BinaryRegressionLink::Probit)),
        ("prais", Method::Prais),
    ] {
        let mut parameters = vec!["constant", "max_iterations", "tolerance"];
        if matches!(method, Method::Prais) {
            parameters.push("transform");
        }
        install(
            builder,
            &format!("yssbi.statistics.{name}.fit"),
            vec![Input::fixed("y"), Input::repeated("x", 1..=usize::MAX)],
            &parameters,
            3,
            move |inv| fit(method, inv),
        );
        install(
            builder,
            &format!("yssbi.statistics.{name}.summary"),
            vec![Input::fixed("model")],
            if matches!(method, Method::Binary(BinaryRegressionLink::Probit)) {
                &[
                    "marginal_effects",
                    "marginal_evaluation",
                    "marginal_method",
                    "marginal_at",
                    "classification",
                    "cutoff",
                    "hypothesis_test",
                    "hypothesis",
                ]
            } else if matches!(method, Method::Binary(_)) {
                &[
                    "odds_ratios",
                    "marginal_effects",
                    "marginal_evaluation",
                    "marginal_method",
                    "marginal_at",
                    "classification",
                    "cutoff",
                    "hypothesis_test",
                    "hypothesis",
                ]
            } else {
                &["hypothesis_test", "hypothesis"]
            },
            1,
            summary,
        );
        if let Method::Binary(link) = method {
            install(
                builder,
                &format!("yssbi.statistics.{name}.predict"),
                vec![Input::fixed("model"), Input::repeated("x", 1..=usize::MAX)],
                &[],
                1,
                move |inv| predict(link, inv),
            );
        }
    }
}

fn fit(method: Method, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let response = data.remove(0);
    let n = response.len();
    let constant = boolean(inv, "constant")?;
    let max_iterations = integer(inv, "max_iterations")?;
    let tolerance = number(inv, "tolerance")?;
    if max_iterations == 0 || tolerance <= 0.0 {
        return Err(KernelError::InvalidParameter);
    }
    check_fit_workspace(n, data.len(), constant, "OLS", inv)?;
    inv.control.check_bytes(
        n.checked_mul(data.len() + usize::from(constant))
            .and_then(|v| v.checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)),
    )?;
    let mut fit = match method {
        Method::Binary(link) => yss_sci_runtime::regression::discrete::fit_binary(
            link,
            response,
            &data,
            BinaryOptions {
                constant,
                max_iterations,
                tolerance,
            },
            metadata(n),
        ),
        Method::Prais => yss_sci_runtime::regression::linear::prais::fit_prais(
            response,
            &data,
            PraisConfig {
                constant,
                max_iter: max_iterations,
                tol: tolerance,
                transform: match text(inv, "transform")? {
                    "prais_winsten" => PraisTransform::PraisWinsten,
                    "cochrane_orcutt" => PraisTransform::CochraneOrcutt,
                    _ => return Err(KernelError::InvalidParameter),
                },
            },
            metadata(n),
        ),
    }
    .map_err(sci)?;
    fit.response_name = input_label(&inv.inputs[0], "response".into());
    fit.parameter_names = std::iter::once("_cons".to_string())
        .take(usize::from(constant))
        .chain(
            inv.inputs
                .iter()
                .skip(1)
                .enumerate()
                .map(|(i, v)| input_label(v, format!("x{}", i + 1))),
        )
        .collect();
    inv.check_control()?;
    let model = value(fit, inv)?;
    Ok(vec![
        model.clone(),
        field(&model, "fitted")?.clone(),
        field(&model, "residuals")?.clone(),
    ])
}

fn summary(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    use yss_sci_contract::regression::{
        discrete::*,
        fit::{RegressionFit, RegressionStatistics},
    };
    let fit: RegressionFit = decode_model(inv)?;
    let mut report = yss_sci_runtime::regression::report::regression_report(&fit).map_err(sci)?;
    if let RegressionStatistics::Binary { link, .. } = fit.statistics {
        check_fit_workspace(fit.fitted.len(), fit.coefficients.len(), false, "OLS", inv)?;
        if link == BinaryRegressionLink::Logit && boolean(inv, "odds_ratios")? {
            report["odds_ratios"] = serde_json::to_value(
                yss_sci_runtime::regression::discrete::odds_ratios(&fit).map_err(sci)?,
            )
            .map_err(|_| KernelError::ScientificFailure)?;
        }
        if boolean(inv, "marginal_effects")? {
            let k = fit.coefficients.len();
            let n = fit.fitted.len();
            inv.control.check_bytes(
                n.checked_mul(k)
                    .and_then(|v| v.checked_add(k.checked_mul(k)?))
                    .and_then(|v| v.checked_mul(16)),
            )?;
            let options = MarginalOptions {
                evaluation: match text(inv, "marginal_evaluation")? {
                    "average" => Evaluation::Average,
                    "at_means" => Evaluation::AtMeans,
                    _ => return Err(KernelError::InvalidParameter),
                },
                method: match text(inv, "marginal_method")? {
                    "dydx" => MarginalMethod::Dydx,
                    "eyex" => MarginalMethod::Eyex,
                    "eydx" => MarginalMethod::Eydx,
                    "dyex" => MarginalMethod::Dyex,
                    _ => return Err(KernelError::InvalidParameter),
                },
                at: yss_sci_runtime::hypothesis::parse_at_values(
                    text(inv, "marginal_at")?,
                    &fit.parameter_names,
                )
                .map_err(|_| KernelError::InvalidParameter)?,
            };
            report["marginal_effects"] = serde_json::to_value(
                yss_sci_runtime::regression::discrete::marginal_effects(
                    &fit,
                    options,
                    &yss_sci_contract::execution::ScientificExecutionControl::from_shared(
                        inv.control.cancellation.clone(),
                        inv.control.deadline,
                    ),
                )
                .map_err(|error| match error {
                    yss_sci_contract::execution::ScientificComputationError::Cancelled => {
                        KernelError::Cancelled
                    }
                    yss_sci_contract::execution::ScientificComputationError::DeadlineExceeded => {
                        KernelError::DeadlineExceeded
                    }
                    _ => KernelError::InvalidParameter,
                })?,
            )
            .map_err(|_| KernelError::ScientificFailure)?;
        }
        if boolean(inv, "classification")? {
            report["classification"] = serde_json::to_value(
                yss_sci_runtime::regression::discrete::classification(&fit, number(inv, "cutoff")?)
                    .map_err(sci)?,
            )
            .map_err(|_| KernelError::ScientificFailure)?;
        }
    }
    if boolean(inv, "hypothesis_test")? {
        let input = yss_sci_contract::hypothesis::HypothesisTestInput {
            betas: fit.coefficients.clone(),
            cov_beta: fit.statistics.coefficient_statistics().covariance.clone(),
            df_residual: match &fit.statistics {
                RegressionStatistics::Prais { model, .. } => model.linear.df_residual,
                RegressionStatistics::Linear { model, .. } => model.df_residual,
                _ => fit
                    .metadata
                    .used_observation_count
                    .saturating_sub(fit.coefficients.len()),
            },
            param_names: fit.parameter_names.clone(),
            hypothesis: text(inv, "hypothesis")?.into(),
        };
        let result = if matches!(fit.statistics, RegressionStatistics::Binary { .. }) {
            yss_sci_runtime::hypothesis::run_asymptotic_hypothesis_test(input)
        } else {
            yss_sci_runtime::hypothesis::run_hypothesis_test(input)
        }
        .map_err(|_| KernelError::InvalidParameter)?;
        report["hypothesis_test"] =
            serde_json::to_value(result).map_err(|_| KernelError::ScientificFailure)?;
    }
    yss_sci_runtime::regression::report::decorate_report(&mut report);
    Ok(vec![value(report, inv)?])
}

fn predict(
    link: BinaryRegressionLink,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let model = inv.inputs.first().ok_or(KernelError::InvalidNumericInput)?;
    let constant = match field(model, "constant")? {
        RuntimeValue::Scalar(TabularScalar::Bool(v)) => *v,
        _ => return Err(KernelError::InvalidNumericInput),
    };
    let expected = match link {
        BinaryRegressionLink::Logit => "logit",
        BinaryRegressionLink::Probit => "probit",
    };
    if !matches!(field(model, "family")?, RuntimeValue::Scalar(TabularScalar::String(v)) if v.as_ref() == expected)
    {
        return Err(KernelError::InvalidNumericInput);
    }
    let coefficients = columns(&[field(model, "coefficients")?], inv, 0)?.remove(0);
    let predictors = columns(&group(inv, "x"), inv, coefficients.len().saturating_mul(8))?;
    check_fit_workspace(predictors[0].len(), predictors.len(), constant, "OLS", inv)?;
    let prediction = yss_sci_runtime::regression::discrete::predict_binary(
        link,
        &coefficients,
        &predictors,
        constant,
    )
    .map_err(sci)?;
    Ok(vec![numeric_list(&prediction, inv)?])
}
