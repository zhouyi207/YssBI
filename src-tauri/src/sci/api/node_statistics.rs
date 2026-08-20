use crate::sci::models::regression::{
    BinaryRegressionLink, BinaryRegressionStatistics, LinearRegressionStatistics, PraisInfo,
    PraisRegressionStatistics, RegressionCoefficientStatistics, RegressionStatistics,
};
use ndarray::{Array1, Array2};
use serde::Serialize;
use statrs::distribution::{ContinuousCDF, Normal};
use yss_sci::regression::discrete::{Logit, LogitConfig, Probit, ProbitConfig};
use yss_sci::regression::linear_model::{
    GLS, GLSConfig, IV2SLS, IV2SLSConfig, IVLIML, IVLIMLConfig, OLS, OLSConfig, Prais, PraisConfig,
    WLS, WLSConfig,
};
use yss_sci::regression::panel::fit_panel_fe_twoway;
use yss_sci::tools::{IntoFaer, matrix_rank};
use yss_sci::ts::unit_root::adf_test;
use yss_sci::ts::var::{VAR, VARConfig, var_varsoc};
use yss_sci::ts::vec::{VECConfig, VecTrendSpec, vec_estimate, vec_vecrank_stats};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressionKind {
    Ols,
    Gls,
    Logit,
    Probit,
    Prais,
    Wls,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionFit {
    pub family: &'static str,
    pub coefficients: Vec<f64>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub statistics: RegressionStatistics,
    pub metadata: crate::sci::models::regression::StatisticalObservationMetadata,
}

pub fn fit_regression(
    kind: RegressionKind,
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    weights: Option<Vec<f64>>,
    metadata: crate::sci::models::regression::StatisticalObservationMetadata,
) -> Result<RegressionFit, String> {
    let y = Array1::from_vec(response);
    let x = design_matrix(&predictors, y.len(), true)?;
    match kind {
        RegressionKind::Ols => {
            let result = OLS {
                endog: y.clone(),
                exog: x.clone(),
                config: OLSConfig {
                    constant: true,
                    cov_type: "nonrobust".into(),
                    cov_params: None,
                },
            }
            .fit()?;
            linear_fit(
                "ols",
                &y,
                &x,
                result.betas.to_vec(),
                RegressionStatistics::Linear {
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.to_vec(),
                        statistic_values: result.tvalues.to_vec(),
                        p_values: result.pvalues.to_vec(),
                        confidence_interval_lower: result.conf_int_left.to_vec(),
                        confidence_interval_upper: result.conf_int_right.to_vec(),
                    },
                    model: LinearRegressionStatistics {
                        r2: result.r2,
                        adjusted_r2: result.r2_adjusted,
                        f_statistic: result.fvalue,
                        f_p_value: result.f_p_value,
                        df_model: result.df_model,
                        df_residual: result.df_residual,
                        df_total: result.df_total,
                        ss_model: result.ss_model,
                        ss_residual: result.ss_residual,
                        ss_total: result.ss_total,
                        ms_model: result.ms_model,
                        ms_residual: result.ms_residual,
                        ms_total: result.ms_total,
                        covariance_type: result.covariance_type,
                        condition_number: result.cond_no,
                    },
                },
                metadata,
            )
        }
        RegressionKind::Gls => {
            let result = GLS {
                endog: y.clone(),
                exog: x.clone(),
                sigma: Array2::eye(y.len()),
                config: GLSConfig { constant: true },
            }
            .fit()?;
            linear_fit(
                "gls",
                &y,
                &x,
                result.betas.to_vec(),
                RegressionStatistics::Linear {
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.to_vec(),
                        statistic_values: result.tvalues.to_vec(),
                        p_values: result.pvalues.to_vec(),
                        confidence_interval_lower: result.conf_int_left.to_vec(),
                        confidence_interval_upper: result.conf_int_right.to_vec(),
                    },
                    model: LinearRegressionStatistics {
                        r2: result.r2,
                        adjusted_r2: result.r2_adjusted,
                        f_statistic: result.fvalue,
                        f_p_value: result.f_p_value,
                        df_model: result.df_model,
                        df_residual: result.df_residual,
                        df_total: result.df_total,
                        ss_model: result.ss_model,
                        ss_residual: result.ss_residual,
                        ss_total: result.ss_total,
                        ms_model: result.ms_model,
                        ms_residual: result.ms_residual,
                        ms_total: result.ms_total,
                        covariance_type: result.covariance_type,
                        condition_number: result.cond_no,
                    },
                },
                metadata,
            )
        }
        RegressionKind::Wls => {
            let weights = weights.ok_or_else(|| "WLS requires a weight series".to_string())?;
            if weights.len() != y.len() {
                return Err("WLS weights must match the response length".into());
            }
            let result = WLS {
                endog: y.clone(),
                exog: x.clone(),
                weights: Array1::from_vec(weights),
                config: WLSConfig {
                    constant: true,
                    cov_type: "nonrobust".into(),
                    cov_params: None,
                },
            }
            .fit()?;
            linear_fit(
                "wls",
                &y,
                &x,
                result.betas.to_vec(),
                RegressionStatistics::Linear {
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.to_vec(),
                        statistic_values: result.tvalues.to_vec(),
                        p_values: result.pvalues.to_vec(),
                        confidence_interval_lower: result.conf_int_left.to_vec(),
                        confidence_interval_upper: result.conf_int_right.to_vec(),
                    },
                    model: LinearRegressionStatistics {
                        r2: result.r2,
                        adjusted_r2: result.r2_adjusted,
                        f_statistic: result.fvalue,
                        f_p_value: result.f_p_value,
                        df_model: result.df_model,
                        df_residual: result.df_residual,
                        df_total: result.df_total,
                        ss_model: result.ss_model,
                        ss_residual: result.ss_residual,
                        ss_total: result.ss_total,
                        ms_model: result.ms_model,
                        ms_residual: result.ms_residual,
                        ms_total: result.ms_total,
                        covariance_type: result.covariance_type,
                        condition_number: result.cond_no,
                    },
                },
                metadata,
            )
        }
        RegressionKind::Prais => {
            let result = Prais {
                endog: y.clone(),
                exog: x.clone(),
                config: PraisConfig::default(),
            }
            .fit()?;
            linear_fit(
                "prais",
                &y,
                &x,
                result.betas.to_vec(),
                RegressionStatistics::Prais {
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.to_vec(),
                        statistic_values: result.tvalues.to_vec(),
                        p_values: result.pvalues.to_vec(),
                        confidence_interval_lower: result.conf_int_left.to_vec(),
                        confidence_interval_upper: result.conf_int_right.to_vec(),
                    },
                    model: PraisRegressionStatistics {
                        linear: LinearRegressionStatistics {
                            r2: result.r2,
                            adjusted_r2: result.r2_adjusted,
                            f_statistic: result.fvalue,
                            f_p_value: result.f_p_value,
                            df_model: result.df_model,
                            df_residual: result.df_residual,
                            df_total: result.df_total,
                            ss_model: result.ss_model,
                            ss_residual: result.ss_residual,
                            ss_total: result.ss_total,
                            ms_model: result.ms_model,
                            ms_residual: result.ms_residual,
                            ms_total: result.ms_total,
                            covariance_type: result.covariance_type,
                            condition_number: result.cond_no,
                        },
                        rho: result.rho,
                        durbin_watson_original: result.dw_original,
                        durbin_watson_transformed: result.dw_transformed,
                        iterations: result.iterations,
                    },
                },
                metadata,
            )
        }
        RegressionKind::Logit => {
            let result = Logit {
                endog: y.clone(),
                exog: x.clone(),
                config: LogitConfig::default(),
            }
            .fit()?;
            let coefficients = result.betas.to_vec();
            let fitted = x
                .dot(&Array1::from_vec(coefficients.clone()))
                .mapv(|value| 1.0 / (1.0 + (-value).exp()))
                .to_vec();
            let adjusted_pseudo_r2 =
                1.0 - (result.log_likelihood - coefficients.len() as f64) / result.ll_null;
            Ok(RegressionFit {
                family: "logit",
                residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
                fitted,
                coefficients,
                statistics: RegressionStatistics::Binary {
                    link: BinaryRegressionLink::Logit,
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.to_vec(),
                        statistic_values: result.zvalues.to_vec(),
                        p_values: result.pvalues.to_vec(),
                        confidence_interval_lower: result.conf_int_left.to_vec(),
                        confidence_interval_upper: result.conf_int_right.to_vec(),
                    },
                    model: BinaryRegressionStatistics {
                        log_likelihood: result.log_likelihood,
                        null_log_likelihood: result.ll_null,
                        pseudo_r2: result.pseudo_r2,
                        adjusted_pseudo_r2,
                        lr_chi2: result.lr_chi2,
                        lr_p_value: result.lr_p_value,
                        aic: result.aic,
                        bic: result.bic,
                        iterations: result.iterations,
                        converged: result.converged,
                        condition_number: design_condition_number(&x),
                    },
                },
                metadata,
            })
        }
        RegressionKind::Probit => {
            let result = Probit {
                endog: y.clone(),
                exog: x.clone(),
                config: ProbitConfig::default(),
            }
            .fit()?;
            let coefficients = result.betas.to_vec();
            let normal = Normal::new(0.0, 1.0).map_err(|error| error.to_string())?;
            let fitted = x
                .dot(&Array1::from_vec(coefficients.clone()))
                .mapv(|value| normal.cdf(value))
                .to_vec();
            let adjusted_pseudo_r2 =
                1.0 - (result.log_likelihood - coefficients.len() as f64) / result.ll_null;
            Ok(RegressionFit {
                family: "probit",
                residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
                fitted,
                coefficients,
                statistics: RegressionStatistics::Binary {
                    link: BinaryRegressionLink::Probit,
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.to_vec(),
                        statistic_values: result.zvalues.to_vec(),
                        p_values: result.pvalues.to_vec(),
                        confidence_interval_lower: result.conf_int_left.to_vec(),
                        confidence_interval_upper: result.conf_int_right.to_vec(),
                    },
                    model: BinaryRegressionStatistics {
                        log_likelihood: result.log_likelihood,
                        null_log_likelihood: result.ll_null,
                        pseudo_r2: result.pseudo_r2,
                        adjusted_pseudo_r2,
                        lr_chi2: result.lr_chi2,
                        lr_p_value: result.lr_p_value,
                        aic: result.aic,
                        bic: result.bic,
                        iterations: result.iterations,
                        converged: result.converged,
                        condition_number: design_condition_number(&x),
                    },
                },
                metadata,
            })
        }
    }
}

fn stable_report_number(value: f64) -> f64 {
    (value * 1e12).round() / 1e12
}

fn covariance_rows(covariance: &Array2<f64>) -> Vec<Vec<f64>> {
    covariance
        .rows()
        .into_iter()
        .map(|row| row.to_vec())
        .collect()
}

fn report_coefficients(fit: &RegressionFit) -> Vec<serde_json::Value> {
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
            serde_json::json!({
                "variable": if index == 0 { "_cons".to_string() } else { format!("x{index}") },
                "coef": coefficient,
                "std_err": statistics.standard_errors[index],
                "t_value": statistics.statistic_values[index],
                "p_value": p_value,
                "confidence_interval_0.025": stable_report_number(statistics.confidence_interval_lower[index]),
                "confidence_interval_0.975": stable_report_number(statistics.confidence_interval_upper[index]),
                "is_significant": p_value < 0.05,
            })
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

fn mean_square(sum_of_squares: f64, degrees_of_freedom: usize) -> f64 {
    if degrees_of_freedom == 0 {
        0.0
    } else {
        sum_of_squares / degrees_of_freedom as f64
    }
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
    let df_total = observations.saturating_sub(1);
    let ss_residual = fit.residuals.iter().map(|value| value * value).sum::<f64>();
    let fitted_mean = fit.fitted.iter().sum::<f64>() / fit.fitted.len() as f64;
    let ss_model = fit
        .fitted
        .iter()
        .map(|value| (value - fitted_mean) * (value - fitted_mean))
        .sum::<f64>();
    let ss_total = ss_model + ss_residual;

    // The current frontend parser requires these legacy slots. Binary views treat
    // them as pseudo-R²/LR aliases, so project real model facts rather than 0/1 defaults.
    serde_json::json!({
        "model_type": match link {
            BinaryRegressionLink::Logit => "Logit",
            BinaryRegressionLink::Probit => "Probit",
        },
        "method": "Maximum Likelihood",
        "num_observation": observations,
        "r_squared": statistics.pseudo_r2,
        "adj_r_squared": statistics.adjusted_pseudo_r2,
        "f_statistic": statistics.lr_chi2,
        "prob_f_statistic": statistics.lr_p_value,
        "wald_chi2": statistics.lr_chi2,
        "prob_wald_chi2": statistics.lr_p_value,
        "log_likelihood": statistics.log_likelihood,
        "lr_chi2": statistics.lr_chi2,
        "prob_lr_chi2": statistics.lr_p_value,
        "df_model": df_model,
        "df_residual": df_residual,
        "df_total": df_total,
        "ss_model": ss_model,
        "ss_residual": ss_residual,
        "ss_total": ss_total,
        "ms_model": mean_square(ss_model, df_model),
        "ms_residual": mean_square(ss_residual, df_residual),
        "ms_total": mean_square(ss_total, df_total),
        "covariance_type": "nonrobust",
        "aic": statistics.aic,
        "bic": statistics.bic,
    })
}

pub fn regression_report(fit: &RegressionFit) -> serde_json::Value {
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
            Some(
                serde_json::to_value(&fit.statistics)
                    .expect("regression statistics must serialize to a report value"),
            ),
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
    let mut report = serde_json::json!({
        "title": format!("{} Summary", fit.family.to_uppercase()),
        "endog_name": "response",
        "model_basic_info": model_basic_info,
        "coefficients": coefficients,
        "diagnostic_info": {
            "cond_no": condition_number,
            "fitted_values": &fit.fitted,
            "residuals": &fit.residuals,
        },
        "betas": &fit.coefficients,
        "cov_beta": &fit.statistics.coefficient_statistics().covariance,
    });
    if let Some(prais_info) = prais_info {
        report["diagnostic_info"]
            .as_object_mut()
            .expect("regression diagnostics must be an object")
            .insert(
                "prais_info".to_owned(),
                serde_json::to_value(prais_info)
                    .expect("Prais statistics must serialize to a report value"),
            );
    }
    if let Some(model_statistics) = model_statistics {
        report
            .as_object_mut()
            .expect("regression report must be an object")
            .insert("model_statistics".to_owned(), model_statistics);
    }
    report
}

fn linear_fit(
    family: &'static str,
    y: &Array1<f64>,
    x: &Array2<f64>,
    coefficients: Vec<f64>,
    statistics: RegressionStatistics,
    metadata: crate::sci::models::regression::StatisticalObservationMetadata,
) -> Result<RegressionFit, String> {
    let fitted = x.dot(&Array1::from_vec(coefficients.clone())).to_vec();
    Ok(RegressionFit {
        family,
        residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
        fitted,
        coefficients,
        statistics,
        metadata,
    })
}

fn design_condition_number(design: &Array2<f64>) -> f64 {
    matrix_rank(design.view().into_faer().to_owned()).1
}

fn design_matrix(
    predictors: &[Vec<f64>],
    observations: usize,
    constant: bool,
) -> Result<Array2<f64>, String> {
    if predictors.is_empty() {
        return Err("regression requires at least one predictor".into());
    }
    if predictors.iter().any(|values| values.len() != observations) {
        return Err("response and predictor lengths must match".into());
    }
    let columns = predictors.len() + usize::from(constant);
    let mut values = Vec::with_capacity(observations * columns);
    for row in 0..observations {
        if constant {
            values.push(1.0);
        }
        for predictor in predictors {
            values.push(predictor[row]);
        }
    }
    Array2::from_shape_vec((observations, columns), values).map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentalVariableKind {
    TwoStageLeastSquares,
    LimitedInformationMaximumLikelihood,
}

pub fn fit_instrumental_variables(
    kind: InstrumentalVariableKind,
    response: Vec<f64>,
    exogenous: Vec<f64>,
    endogenous: Vec<f64>,
    instruments: Vec<f64>,
) -> Result<serde_json::Value, String> {
    let observations = response.len();
    if [exogenous.len(), endogenous.len(), instruments.len()]
        .into_iter()
        .any(|len| len != observations)
    {
        return Err("IV series lengths must match".into());
    }
    let column = |values: Vec<f64>| {
        Array2::from_shape_vec((observations, 1), values).map_err(|error| error.to_string())
    };
    match kind {
        InstrumentalVariableKind::TwoStageLeastSquares => {
            let result = IV2SLS {
                endog: Array1::from_vec(response),
                exog: column(exogenous)?,
                endog_reg: column(endogenous)?,
                instruments: column(instruments)?,
                config: IV2SLSConfig {
                    constant: true,
                    cov_type: "nonrobust".into(),
                    cov_params: None,
                    small: false,
                },
                endog_names: None,
                z_var_names: None,
            }
            .fit()?;
            Ok(serde_json::json!({
                "family": "iv_2sls",
                "coefficients": result.betas.to_vec(),
                "standardErrors": result.stds.to_vec(),
                "pValues": result.pvalues.to_vec(),
                "r2": result.r2,
                "adjustedR2": result.r2_adjusted,
                "firstStageMinEigenvalue": result.first_stage_summary.min_eigenvalue,
            }))
        }
        InstrumentalVariableKind::LimitedInformationMaximumLikelihood => {
            let result = IVLIML {
                endog: Array1::from_vec(response),
                exog: column(exogenous)?,
                endog_reg: column(endogenous)?,
                instruments: column(instruments)?,
                config: IVLIMLConfig {
                    constant: true,
                    cov_type: "nonrobust".into(),
                    cov_params: None,
                    small: false,
                },
                endog_names: None,
                z_var_names: None,
            }
            .fit()?;
            Ok(serde_json::json!({
                "family": "iv_liml",
                "coefficients": result.betas.to_vec(),
                "standardErrors": result.stds.to_vec(),
                "pValues": result.pvalues.to_vec(),
                "r2": result.r2,
                "adjustedR2": result.r2_adjusted,
                "kappa": result.kappa,
                "firstStageMinEigenvalue": result.first_stage_summary.min_eigenvalue,
            }))
        }
    }
}

pub fn fit_panel(
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    entity: Vec<f64>,
    time: Vec<f64>,
    treatment: Option<Vec<f64>>,
) -> Result<serde_json::Value, String> {
    let observations = response.len();
    if entity.len() != observations || time.len() != observations {
        return Err("panel entity/time series lengths must match the response".into());
    }
    let mut predictors = predictors;
    let is_did = treatment.is_some();
    if let Some(treatment) = treatment {
        if treatment.len() != observations {
            return Err("panel treatment length must match the response".into());
        }
        predictors.push(treatment);
    }
    let exog = design_matrix(&predictors, observations, false)?;
    let ids = |values: Vec<f64>| -> Vec<usize> {
        let mut levels = Vec::<f64>::new();
        values
            .into_iter()
            .map(|value| {
                levels
                    .iter()
                    .position(|level| *level == value)
                    .unwrap_or_else(|| {
                        levels.push(value);
                        levels.len() - 1
                    })
            })
            .collect()
    };
    let result = fit_panel_fe_twoway(
        &Array1::from_vec(response),
        &exog,
        &ids(entity),
        &ids(time),
        true,
        "cluster",
        None,
    )?;
    Ok(serde_json::json!({
        "family": if is_did { "panel_did_twfe" } else { "panel_fe_twoway" },
        "coefficients": result.betas.to_vec(),
        "standardErrors": result.stds.to_vec(),
        "pValues": result.pvalues.to_vec(),
        "r2": result.r2,
        "adjustedR2": result.r2_adjusted,
        "observations": result.num_observation,
        "entities": result.num_entities,
        "timePeriods": result.num_time_periods,
    }))
}

pub fn augmented_dickey_fuller(
    series: &[f64],
    lags: usize,
    regression: &str,
) -> Result<serde_json::Value, String> {
    let (constant, trend) = match regression {
        "none" | "no_constant" => (false, false),
        "constant" => (true, false),
        "trend" => (true, true),
        other => return Err(format!("unsupported ADF regression '{other}'")),
    };
    let result = adf_test(series, lags, constant, trend)?;
    serde_json::to_value(serde_json::json!({
        "operation": "adf",
        "statistic": result.test_statistic,
        "pValue": result.p_value,
        "observations": result.num_obs,
        "lags": result.lags,
        "criticalValues": {
            "1%": result.critical_value_1pct,
            "5%": result.critical_value_5pct,
            "10%": result.critical_value_10pct,
        }
    }))
    .map_err(|error| error.to_string())
}

fn multivariate_series(series: Vec<Vec<f64>>) -> Result<Array2<f64>, String> {
    let observations = series.first().map(Vec::len).unwrap_or(0);
    if series.len() < 2 || observations == 0 || series.iter().any(|item| item.len() != observations)
    {
        return Err(
            "multivariate time-series inputs must contain at least two equal-length series".into(),
        );
    }
    let mut values = Vec::with_capacity(observations * series.len());
    for row in 0..observations {
        for column in &series {
            values.push(column[row]);
        }
    }
    Array2::from_shape_vec((observations, series.len()), values).map_err(|error| error.to_string())
}

pub fn var_fit(series: Vec<Vec<f64>>, lags: usize) -> Result<serde_json::Value, String> {
    if lags == 0 {
        return Err("VAR lags must be positive".into());
    }
    let y = multivariate_series(series)?;
    let result = VAR {
        y,
        exog: None,
        config: VARConfig {
            constant: true,
            lags: (1..=lags).collect(),
            step: 8,
            dfk: false,
            mlag: 2,
            sample_start_offset: None,
            skip_extras: false,
        },
        var_names: None,
        exog_names: None,
        regression_times: None,
    }
    .fit()?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

pub fn var_lag_order(series: Vec<Vec<f64>>, max_lags: usize) -> Result<serde_json::Value, String> {
    serde_json::to_value(var_varsoc(multivariate_series(series)?, max_lags, None)?)
        .map_err(|error| error.to_string())
}

pub fn vec_fit(
    series: Vec<Vec<f64>>,
    rank: usize,
    lags: usize,
    trend: &str,
) -> Result<serde_json::Value, String> {
    let result = vec_estimate(
        &multivariate_series(series)?,
        &VECConfig {
            trend_spec: vec_trend(trend)?,
            lags,
            rank,
            mlag: 2,
        },
        None,
        None,
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

pub fn vec_rank_test(
    series: Vec<Vec<f64>>,
    lags: usize,
    trend: &str,
) -> Result<serde_json::Value, String> {
    let result = vec_vecrank_stats(
        &multivariate_series(series)?,
        lags,
        vec_trend(trend)?,
        None,
        true,
        None,
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

fn vec_trend(trend: &str) -> Result<VecTrendSpec, String> {
    match trend {
        "none" | "no_constant" => Ok(VecTrendSpec::None),
        "constant" => Ok(VecTrendSpec::Constant),
        "trend" => Ok(VecTrendSpec::Trend),
        other => Err(format!("unsupported VEC trend '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sci::models::regression::{
        StatisticalObservationMetadata, StatisticalSettingSource,
    };

    fn regression_metadata(observations: usize) -> StatisticalObservationMetadata {
        StatisticalObservationMetadata {
            original_observation_count: observations,
            used_observation_count: observations,
            dropped_null_count: 0,
            dropped_nan_count: 0,
            missing_value_policy: crate::project::StatisticalMissingValuePolicy::Listwise,
            missing_value_policy_source: StatisticalSettingSource::Project,
            effective_convergence_tolerance: 1e-12,
            convergence_tolerance_source: StatisticalSettingSource::Project,
            convergence_tolerance_consumed: false,
        }
    }

    #[test]
    fn regression_reports_expose_hypothesis_inputs() {
        let response = vec![1.0, 2.1, 2.9, 4.2, 5.1, 5.9];
        let predictor = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let fit = fit_regression(
            RegressionKind::Ols,
            response.clone(),
            vec![predictor],
            None,
            regression_metadata(response.len()),
        )
        .unwrap();

        let report = regression_report(&fit);

        assert_eq!(report["betas"], serde_json::json!(fit.coefficients));
        assert_eq!(report["cov_beta"].as_array().map(Vec::len), Some(2));
        assert!(
            report["cov_beta"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row.as_array().map(Vec::len) == Some(2))
        );
    }

    #[test]
    fn binary_regression_reports_preserve_likelihood_statistics() {
        let response = vec![0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0];
        let predictor = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

        for (kind, expected_link) in [
            (RegressionKind::Logit, "logit"),
            (RegressionKind::Probit, "probit"),
        ] {
            let fit = fit_regression(
                kind,
                response.clone(),
                vec![predictor.clone()],
                None,
                regression_metadata(response.len()),
            )
            .unwrap();
            let report = regression_report(&fit);
            let basic = report["model_basic_info"]
                .as_object()
                .expect("binary report must keep the canonical model info object");

            for field in [
                "num_observation",
                "r_squared",
                "adj_r_squared",
                "f_statistic",
                "prob_f_statistic",
                "df_model",
                "df_residual",
                "df_total",
                "ss_model",
                "ss_residual",
                "ss_total",
                "ms_model",
                "ms_residual",
                "ms_total",
            ] {
                assert!(
                    basic[field].as_f64().is_some_and(f64::is_finite),
                    "canonical model info field {field} must be finite"
                );
            }
            for field in ["model_type", "method", "covariance_type"] {
                assert!(basic[field].as_str().is_some(), "missing {field}");
            }
            assert!(
                report["diagnostic_info"]["cond_no"]
                    .as_f64()
                    .is_some_and(f64::is_finite),
                "binary report must expose a real condition number"
            );
            assert_ne!(basic["r_squared"], serde_json::json!(0.0));
            assert_ne!(basic["f_statistic"], serde_json::json!(0.0));
            assert_ne!(basic["prob_f_statistic"], serde_json::json!(1.0));
            assert_eq!(basic["f_statistic"], basic["lr_chi2"]);
            assert_eq!(basic["prob_f_statistic"], basic["prob_lr_chi2"]);
            for field in ["log_likelihood", "lr_chi2", "prob_lr_chi2", "aic", "bic"] {
                assert!(basic[field].as_f64().is_some(), "missing {field}");
            }

            let statistics = report["model_statistics"]
                .as_object()
                .expect("binary report must expose structured model statistics");
            assert_eq!(statistics["kind"], "binary");
            assert_eq!(statistics["link"], expected_link);
            assert_eq!(statistics["pseudoR2"], basic["r_squared"]);
            assert_eq!(statistics["logLikelihood"], basic["log_likelihood"]);
            assert_eq!(statistics["lrChi2"], basic["lr_chi2"]);
            assert_eq!(statistics["lrPValue"], basic["prob_lr_chi2"]);
            assert_eq!(statistics["aic"], basic["aic"]);
            assert_eq!(statistics["bic"], basic["bic"]);
            assert!(statistics["iterations"].as_u64().unwrap() > 0);
            assert_eq!(statistics["converged"], true);
            let decoded: RegressionStatistics =
                serde_json::from_value(report["model_statistics"].clone()).unwrap();
            assert_eq!(decoded, fit.statistics);
        }
    }

    #[test]
    fn prais_regression_report_preserves_autocorrelation_statistics() {
        let response = vec![1.0, 1.8, 2.7, 3.9, 5.4, 6.8, 8.5, 10.1];
        let predictor = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let fit = fit_regression(
            RegressionKind::Prais,
            response.clone(),
            vec![predictor],
            None,
            regression_metadata(response.len()),
        )
        .unwrap();

        let report = regression_report(&fit);
        let prais = report["diagnostic_info"]["prais_info"]
            .as_object()
            .expect("Prais report must expose structured autocorrelation statistics");

        assert!(prais["rho"].as_f64().is_some());
        assert!(prais["dw_original"].as_f64().is_some());
        assert!(prais["dw_transformed"].as_f64().is_some());
        assert!(prais["iterations"].as_u64().unwrap() > 0);
    }

    #[test]
    fn augmented_dickey_fuller_rejects_unknown_regression() {
        let series = [1.0, 1.4, 1.1, 1.8, 1.5, 2.2, 1.9, 2.6, 2.3, 3.0, 2.7, 3.4];

        let error = augmented_dickey_fuller(&series, 1, "unexpected").unwrap_err();

        assert_eq!(error, "unsupported ADF regression 'unexpected'");
    }
}
