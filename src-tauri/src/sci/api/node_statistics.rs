use ndarray::{Array1, Array2};
use serde::Serialize;
use statrs::distribution::{ContinuousCDF, Normal};
use yss_sci::regression::discrete::{Logit, LogitConfig, Probit, ProbitConfig};
use yss_sci::regression::linear_model::{
    GLS, GLSConfig, IV2SLS, IV2SLSConfig, IVLIML, IVLIMLConfig, OLS, OLSConfig, Prais, PraisConfig,
    WLS, WLSConfig,
};
use yss_sci::regression::panel::fit_panel_fe_twoway;
use yss_sci::ts::unit_root::adf_test;
use yss_sci::ts::var::var_varsoc;
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
pub struct RegressionFit {
    pub family: &'static str,
    pub coefficients: Vec<f64>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub statistics: serde_json::Value,
}

pub fn fit_regression(
    kind: RegressionKind,
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    weights: Option<Vec<f64>>,
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
                serde_json::json!({
                    "r2": result.r2,
                    "adjustedR2": result.r2_adjusted,
                    "fStatistic": result.fvalue,
                    "fPValue": result.f_p_value,
                    "standardErrors": result.stds.to_vec(),
                    "pValues": result.pvalues.to_vec(),
                }),
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
                serde_json::json!({
                    "r2": result.r2,
                    "adjustedR2": result.r2_adjusted,
                    "fStatistic": result.fvalue,
                    "fPValue": result.f_p_value,
                    "standardErrors": result.stds.to_vec(),
                    "pValues": result.pvalues.to_vec(),
                }),
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
                serde_json::json!({
                    "r2": result.r2,
                    "adjustedR2": result.r2_adjusted,
                    "fStatistic": result.fvalue,
                    "fPValue": result.f_p_value,
                    "standardErrors": result.stds.to_vec(),
                    "pValues": result.pvalues.to_vec(),
                }),
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
                serde_json::json!({
                    "rho": result.rho,
                    "durbinWatsonOriginal": result.dw_original,
                    "durbinWatsonTransformed": result.dw_transformed,
                    "iterations": result.iterations,
                    "standardErrors": result.stds.to_vec(),
                    "pValues": result.pvalues.to_vec(),
                }),
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
            Ok(RegressionFit {
                family: "logit",
                residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
                fitted,
                coefficients,
                statistics: serde_json::json!({
                    "logLikelihood": result.log_likelihood,
                    "pseudoR2": result.pseudo_r2,
                    "lrChi2": result.lr_chi2,
                    "lrPValue": result.lr_p_value,
                    "aic": result.aic,
                    "bic": result.bic,
                    "iterations": result.iterations,
                    "converged": result.converged,
                    "standardErrors": result.stds.to_vec(),
                    "pValues": result.pvalues.to_vec(),
                }),
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
            Ok(RegressionFit {
                family: "probit",
                residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
                fitted,
                coefficients,
                statistics: serde_json::json!({
                    "logLikelihood": result.log_likelihood,
                    "pseudoR2": result.pseudo_r2,
                    "lrChi2": result.lr_chi2,
                    "lrPValue": result.lr_p_value,
                    "aic": result.aic,
                    "bic": result.bic,
                    "iterations": result.iterations,
                    "converged": result.converged,
                    "standardErrors": result.stds.to_vec(),
                    "pValues": result.pvalues.to_vec(),
                }),
            })
        }
    }
}

fn linear_fit(
    family: &'static str,
    y: &Array1<f64>,
    x: &Array2<f64>,
    coefficients: Vec<f64>,
    statistics: serde_json::Value,
) -> Result<RegressionFit, String> {
    let fitted = x.dot(&Array1::from_vec(coefficients.clone())).to_vec();
    Ok(RegressionFit {
        family,
        residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
        fitted,
        coefficients,
        statistics,
    })
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
        "trend" => (true, true),
        _ => (true, false),
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
