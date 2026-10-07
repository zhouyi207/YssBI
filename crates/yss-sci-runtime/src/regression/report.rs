use crate::error::computation_failed;
use serde::Serialize;
use yss_sci_contract::regression::fit::{
    BinaryRegressionLink, BinaryRegressionStatistics, LinearRegressionStatistics, RegressionFit,
    RegressionStatistics,
};
use yss_sci_contract::regression::report::{
    LinearDiagnostics, LinearModelSummary, LinearRegressionSummary, RegressionCoefficient,
};
use yss_sci_contract::{SciError, SciOperationCode};

/// Prais-Winsten / Cochrane-Orcutt 特有诊断信息
#[derive(Serialize)]
struct PraisInfo {
    pub rho: f64,
    pub dw_original: f64,
    pub dw_transformed: f64,
    pub iterations: usize,
    /// Iteration log: "Prais iteration N: rho = X.XXXX" for each step
    pub iteration_log: Vec<String>,
    pub rho_history: Vec<f64>,
    pub transform: String,
}

fn stable_report_number(value: f64) -> f64 {
    (value * 1e12).round() / 1e12
}

fn report_coefficients(fit: &RegressionFit) -> Vec<RegressionCoefficient> {
    let statistics = fit.statistics.coefficient_statistics();

    fit.coefficients
        .iter()
        .enumerate()
        .map(|(index, coefficient)| {
            let p_value = statistics.p_values[index];
            RegressionCoefficient {
                variable: fit.parameter_names[index].clone(),
                coef: *coefficient,
                std_err: statistics.standard_errors[index],
                t_value: statistics.statistic_values[index],
                p_value,
                ci_lower: stable_report_number(statistics.confidence_interval_lower[index]),
                ci_upper: stable_report_number(statistics.confidence_interval_upper[index]),
                is_significant: p_value < 0.05,
            }
        })
        .collect()
}

fn linear_model_basic_info(
    family: &str,
    observations: usize,
    statistics: &LinearRegressionStatistics,
) -> serde_json::Value {
    serde_json::json!({
        "model_type": family.to_uppercase(),
        "method": "Least Squares",
        "num_observation": observations,
        "r_squared": statistics.r2,
        "adj_r_squared": statistics.adjusted_r2,
        "f_statistic": statistics.f_statistic,
        "prob_f_statistic": statistics.f_p_value,
        "df_model": statistics.df_model,
        "df_residual": statistics.df_residual,
        "df_total": statistics.df_total,
        "ss_model": statistics.ss_model,
        "ss_residual": statistics.ss_residual,
        "ss_total": statistics.ss_total,
        "ms_model": statistics.ms_model,
        "ms_residual": statistics.ms_residual,
        "ms_total": statistics.ms_total,
        "covariance_type": statistics.covariance_type,
    })
}

fn binary_model_basic_info(
    fit: &RegressionFit,
    link: BinaryRegressionLink,
    statistics: &BinaryRegressionStatistics,
) -> serde_json::Value {
    let observations = fit.metadata.used_observation_count;
    let parameters = fit.coefficients.len();
    let df_model = parameters.saturating_sub(usize::from(fit.constant));
    let df_residual = observations.saturating_sub(parameters);
    serde_json::json!({
        "model_type": match link {
            BinaryRegressionLink::Logit => "Logit",
            BinaryRegressionLink::Probit => "Probit",
        },
        "method": "Maximum Likelihood",
        "num_observation": observations,
        "pseudo_r2": statistics.pseudo_r2,
        "adjusted_pseudo_r2": statistics.adjusted_pseudo_r2,
        "log_likelihood": statistics.log_likelihood,
        "lr_chi2": statistics.lr_chi2,
        "prob_lr_chi2": statistics.lr_p_value,
        "df_model": df_model,
        "df_residual": df_residual,
        "covariance_type": "nonrobust",
        "aic": statistics.aic,
        "bic": statistics.bic,
    })
}

pub fn regression_report(fit: &RegressionFit) -> Result<serde_json::Value, SciError> {
    validate_report_fit(fit)?;
    if matches!(fit.family.as_str(), "ols" | "wls" | "gls") {
        return serde_json::to_value(linear_regression_report(fit)?)
            .map_err(|_| computation_failed(SciOperationCode::Regression));
    }
    #[derive(Serialize)]
    struct DiagnosticInfo<'a> {
        cond_no: f64,
        fitted_values: &'a [f64],
        residuals: &'a [f64],
        #[serde(skip_serializing_if = "Option::is_none")]
        prais_info: Option<PraisInfo>,
    }

    #[derive(Serialize)]
    struct RegressionReport<'a> {
        title: String,
        endog_name: &'a str,
        statistic_distribution: &'static str,
        model_equation: String,
        model_basic_info: serde_json::Value,
        coefficients: Vec<RegressionCoefficient>,
        diagnostic_info: DiagnosticInfo<'a>,
        betas: &'a [f64],
        cov_beta: &'a [Vec<f64>],
        #[serde(skip_serializing_if = "Option::is_none")]
        model_statistics: Option<&'a RegressionStatistics>,
    }

    let observations = fit.metadata.used_observation_count;
    let coefficients = report_coefficients(fit);
    let (model_basic_info, condition_number, model_statistics, prais_info) = match &fit.statistics {
        RegressionStatistics::Linear { model, .. } => (
            linear_model_basic_info(&fit.family, observations, model),
            model.condition_number,
            None,
            None,
        ),
        RegressionStatistics::Binary { link, model, .. } => (
            binary_model_basic_info(fit, *link, model),
            model.condition_number,
            Some(&fit.statistics),
            None,
        ),
        RegressionStatistics::Prais { model, .. } => (
            linear_model_basic_info(&fit.family, observations, &model.linear),
            model.linear.condition_number,
            None,
            Some(PraisInfo {
                rho: model.rho,
                dw_original: model.durbin_watson_original,
                dw_transformed: model.durbin_watson_transformed,
                iterations: model.iterations,
                iteration_log: model.iteration_log.clone(),
                rho_history: model.rho_history.clone(),
                transform: model.transform.clone(),
            }),
        ),
    };
    serde_json::to_value(RegressionReport {
        title: format!("{} Summary", fit.family.to_uppercase()),
        endog_name: &fit.response_name,
        model_equation: crate::report_display::equation(
            &match fit.family.as_str() {
                "logit" => format!("logit(P({}=1))", fit.response_name),
                "probit" => format!("Φ⁻¹(P({}=1))", fit.response_name),
                _ => fit.response_name.clone(),
            },
            &fit.parameter_names,
            &fit.coefficients,
        ),
        statistic_distribution: if matches!(fit.statistics, RegressionStatistics::Binary { .. }) {
            "z"
        } else {
            "t"
        },
        model_basic_info,
        coefficients,
        diagnostic_info: DiagnosticInfo {
            cond_no: condition_number,
            fitted_values: &fit.fitted,
            residuals: &fit.residuals,
            prais_info,
        },
        betas: &fit.coefficients,
        cov_beta: &fit.statistics.coefficient_statistics().covariance,
        model_statistics,
    })
    .map_err(|_| computation_failed(SciOperationCode::Regression))
}

pub fn linear_regression_report(fit: &RegressionFit) -> Result<LinearRegressionSummary, SciError> {
    validate_report_fit(fit)?;
    let RegressionStatistics::Linear { model, .. } = &fit.statistics else {
        return Err(computation_failed(SciOperationCode::Regression));
    };
    Ok(LinearRegressionSummary {
        title: "Linear Regression Summary".into(),
        endog_name: fit.response_name.clone(),
        model_basic_info: LinearModelSummary {
            model_type: fit.family.to_uppercase(),
            method: match fit.family.as_str() {
                "ols" => "Ordinary Least Squares",
                "wls" => "Weighted Least Squares",
                "gls" => "Generalized Least Squares",
                _ => return Err(computation_failed(SciOperationCode::Regression)),
            }
            .into(),
            num_observation: fit.metadata.used_observation_count,
            r_squared: model.r2,
            adj_r_squared: model.adjusted_r2,
            f_statistic: model.f_statistic,
            prob_f_statistic: model.f_p_value,
            df_model: model.df_model,
            df_residual: model.df_residual,
            df_total: model.df_total,
            ss_model: model.ss_model,
            ss_residual: model.ss_residual,
            ss_total: model.ss_total,
            ms_model: model.ms_model,
            ms_residual: model.ms_residual,
            ms_total: model.ms_total,
            covariance_type: model.covariance_type.clone(),
        },
        coefficients: report_coefficients(fit),
        diagnostic_info: LinearDiagnostics {
            cond_no: model.condition_number,
        },
        cov_beta: fit.statistics.coefficient_statistics().covariance.clone(),
    })
}

/// Add declarative sections after selected analyses have been assembled.
pub fn decorate_report(report: &mut serde_json::Value) {
    use crate::report_display::section;
    let z = report["statistic_distribution"] == "z";
    section(
        report,
        "equation",
        "Model equation",
        "equation",
        "/model_equation",
        &[],
    );
    section(
        report,
        "coefficients",
        "Coefficients",
        "table",
        "/coefficients",
        &[
            ("variable", "Variable"),
            ("coef", "Coefficient"),
            ("std_err", "Std. error"),
            ("t_value", if z { "z" } else { "t" }),
            ("p_value", "p-value"),
            ("confidence_interval_0.025", "95% CI lower"),
            ("confidence_interval_0.975", "95% CI upper"),
        ],
    );
    for (key, title) in [
        ("odds_ratios", "Odds ratios (Logit)"),
        ("marginal_effects", "Marginal effects"),
    ] {
        if report.get(key).is_some() {
            let path = if key == "marginal_effects" {
                "/marginal_effects/coefficients"
            } else {
                "/odds_ratios"
            };
            section(
                report,
                key,
                title,
                "table",
                path,
                &[
                    ("variable", "Variable"),
                    ("estimate", "Estimate"),
                    ("standard_error", "Delta-method SE"),
                    ("z_value", "z"),
                    ("p_value", "p-value"),
                    ("ci_lower", "95% CI lower"),
                    ("ci_upper", "95% CI upper"),
                ],
            );
        }
    }
    if let Some(c) = report.get("classification").cloned() {
        report["classification_table"] = serde_json::json!([c]);
        section(
            report,
            "classification",
            "Classification (estimation sample)",
            "table",
            "/classification_table",
            &[
                ("cutoff", "Cutoff"),
                ("true_positive", "TP"),
                ("false_positive", "FP"),
                ("false_negative", "FN"),
                ("true_negative", "TN"),
                ("sensitivity", "Sensitivity"),
                ("specificity", "Specificity"),
                ("positive_predictive_value", "PPV"),
                ("negative_predictive_value", "NPV"),
                ("accuracy", "Accuracy"),
                ("error_rate", "Error rate"),
            ],
        );
    }
    if let Some(h) = report.get("hypothesis_test").cloned() {
        report["hypothesis_table"] = serde_json::json!([h]);
        section(
            report,
            "hypothesis",
            "Coefficient restrictions",
            "table",
            "/hypothesis_table",
            &[
                ("h0_form", "Null hypothesis"),
                ("test_type", "Distribution"),
                ("stat", "Statistic"),
                ("df1", "df1"),
                ("df2", "df2 (0 = asymptotic)"),
                ("p_value", "p-value"),
            ],
        );
    }
    if let Some(rows) = report["diagnostic_info"]["prais_info"]["rho_history"].as_array() {
        report["iteration_table"] = serde_json::json!(
            rows.iter()
                .enumerate()
                .map(|(i, v)| serde_json::json!({"iteration":i+1,"rho":v}))
                .collect::<Vec<_>>()
        );
        report["ar1_parameter_rows"] = serde_json::json!([{"parameter":"rho","estimate":report["diagnostic_info"]["prais_info"]["rho"],"standard_error":null,"inference":"rho standard error not computed","transform":report["diagnostic_info"]["prais_info"]["transform"]}]);
        section(
            report,
            "ar1_parameter",
            "AR(1) parameter",
            "table",
            "/ar1_parameter_rows",
            &[
                ("parameter", "Parameter"),
                ("estimate", "Estimate"),
                ("standard_error", "Std. error"),
                ("inference", "Inference availability"),
                ("transform", "Transformation"),
            ],
        );
        report["ar1_equation"] = serde_json::json!(format!(
            "u[t] = {} × u[t-1] + ε[t]; transformation: {}",
            report["diagnostic_info"]["prais_info"]["rho"],
            report["diagnostic_info"]["prais_info"]["transform"]
                .as_str()
                .unwrap_or("")
        ));
        section(
            report,
            "iterations",
            "AR(1) iterations",
            "table",
            "/iteration_table",
            &[("iteration", "Iteration"), ("rho", "Rho")],
        );
        section(
            report,
            "ar1_equation",
            "AR(1) error model",
            "equation",
            "/ar1_equation",
            &[],
        );
    }
}

fn validate_report_fit(fit: &RegressionFit) -> Result<(), SciError> {
    if fit.parameter_names.len() != fit.coefficients.len()
        || !fit
            .statistics
            .coefficient_statistics()
            .has_shape(fit.coefficients.len())
    {
        return Err(yss_sci_contract::SciError::InvalidInput {
            operation: SciOperationCode::Regression,
            violation: yss_sci_contract::execution::ScientificInputViolation::ShapeMismatch,
        });
    }
    Ok(())
}
