use super::RegressionFit;
use super::types::{
    BinaryRegressionLink, BinaryRegressionStatistics, LinearRegressionStatistics, PraisInfo,
    RegressionStatistics,
};
use crate::error::computation_failed;
use serde::Serialize;
use yss_sci_contract::regression::report::{
    OlsDiagnostics, OlsModelSummary, OlsSummary, RegressionCoefficient,
};
use yss_sci_contract::{SciError, SciOperationCode};

fn stable_report_number(value: f64) -> f64 {
    (value * 1e12).round() / 1e12
}

fn report_coefficients(fit: &RegressionFit) -> Vec<RegressionCoefficient> {
    let statistics = fit.statistics.coefficient_statistics();
    let expected = fit.coefficients.len();
    let lengths = [
        statistics.standard_errors.len(),
        statistics.statistic_values.len(),
        statistics.p_values.len(),
        statistics.confidence_interval_lower.len(),
        statistics.confidence_interval_upper.len(),
    ];
    assert!(
        lengths.into_iter().all(|length| length == expected),
        "regression report requires coefficient statistics for all {expected} coefficients; got {lengths:?}"
    );
    assert!(
        statistics.covariance.len() == expected
            && statistics
                .covariance
                .iter()
                .all(|row| row.len() == expected),
        "regression report requires a {expected}x{expected} coefficient covariance matrix"
    );

    fit.coefficients
        .iter()
        .enumerate()
        .map(|(index, coefficient)| {
            let p_value = statistics.p_values[index];
            RegressionCoefficient {
                variable: if fit.constant && index == 0 {
                    "_cons".to_string()
                } else {
                    format!("x{}", index + usize::from(!fit.constant))
                },
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
    let df_model = parameters.saturating_sub(1);
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
    if fit.family == "ols" {
        return serde_json::to_value(ols_report(fit)?)
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
        endog_name: &'static str,
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
            linear_model_basic_info(fit.family, observations, model),
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
            linear_model_basic_info(fit.family, observations, &model.linear),
            model.linear.condition_number,
            None,
            Some(PraisInfo {
                rho: model.rho,
                dw_original: model.durbin_watson_original,
                dw_transformed: model.durbin_watson_transformed,
                iterations: model.iterations,
                iteration_log: Vec::new(),
            }),
        ),
    };
    serde_json::to_value(RegressionReport {
        title: format!("{} Summary", fit.family.to_uppercase()),
        endog_name: "response",
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

pub fn ols_report(fit: &RegressionFit) -> Result<OlsSummary, SciError> {
    let RegressionStatistics::Linear { model, .. } = &fit.statistics else {
        return Err(computation_failed(SciOperationCode::Regression));
    };
    Ok(OlsSummary {
        title: "OLS Summary".into(),
        endog_name: "response".into(),
        model_basic_info: OlsModelSummary {
            model_type: "OLS".into(),
            method: "Least Squares".into(),
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
        diagnostic_info: OlsDiagnostics {
            cond_no: model.condition_number,
        },
        cov_beta: fit.statistics.coefficient_statistics().covariance.clone(),
    })
}
