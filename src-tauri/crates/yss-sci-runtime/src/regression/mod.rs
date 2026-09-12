use crate::error::{computation_failed, invalid_input};
use serde::Serialize;
use yss_sci_contract::regression::OlsOptions;
pub mod report;
pub mod types;
use crate::regression::types::{
    BinaryRegressionLink, BinaryRegressionStatistics, LinearRegressionStatistics,
    PraisRegressionStatistics, RegressionCoefficientStatistics, RegressionStatistics,
};
use faer::{Col, Mat};
use statrs::distribution::{ContinuousCDF, Normal};
use yss_linalg::matrix_rank;
use yss_sci::regression::discrete::{Logit, LogitConfig, Probit, ProbitConfig};
use yss_sci::regression::linear_model::{
    GLS, GLSConfig, IV2SLS, IV2SLSConfig, IVLIML, IVLIMLConfig, OLS, Prais, PraisConfig, WLS,
    WLSConfig,
};
use yss_sci::regression::panel::fit_panel_fe_twoway;
use yss_sci_contract::{
    SciError, SciInputViolation, SciOperationCode, StatisticalObservationMetadata,
};

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
    pub constant: bool,
    pub family: &'static str,
    pub coefficients: Vec<f64>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub statistics: RegressionStatistics,
    pub metadata: StatisticalObservationMetadata,
}

pub fn fit_regression(
    kind: RegressionKind,
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    weights: Option<Vec<f64>>,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    let y = Col::from_iter(response);
    let x = design_matrix(&predictors, y.nrows(), true, SciOperationCode::Regression)?;
    match kind {
        RegressionKind::Ols => fit_ols_design(&y, &x, &OlsOptions::default(), metadata),
        RegressionKind::Gls => {
            let result = GLS {
                endog: y.clone(),
                exog: x.clone(),
                sigma: Mat::identity(y.nrows(), y.nrows()),
                config: GLSConfig { constant: true },
            }
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::Regression))?;
            linear_fit(
                "gls",
                &y,
                &x,
                result.betas.iter().copied().collect::<Vec<_>>(),
                RegressionStatistics::Linear {
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.iter().copied().collect::<Vec<_>>(),
                        statistic_values: result.tvalues.iter().copied().collect::<Vec<_>>(),
                        p_values: result.pvalues.iter().copied().collect::<Vec<_>>(),
                        confidence_interval_lower: result
                            .conf_int_left
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
                        confidence_interval_upper: result
                            .conf_int_right
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
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
            let weights = weights.ok_or_else(|| {
                invalid_input(SciOperationCode::Regression, SciInputViolation::EmptyInput)
            })?;
            if weights.len() != y.nrows() {
                return Err(invalid_input(
                    SciOperationCode::Regression,
                    SciInputViolation::ShapeMismatch,
                ));
            }
            let result = WLS {
                endog: y.clone(),
                exog: x.clone(),
                weights: Col::from_iter(weights),
                config: WLSConfig {
                    constant: true,
                    cov_type: "nonrobust".into(),
                    cov_params: None,
                },
            }
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::Regression))?;
            linear_fit(
                "wls",
                &y,
                &x,
                result.betas.iter().copied().collect::<Vec<_>>(),
                RegressionStatistics::Linear {
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.iter().copied().collect::<Vec<_>>(),
                        statistic_values: result.tvalues.iter().copied().collect::<Vec<_>>(),
                        p_values: result.pvalues.iter().copied().collect::<Vec<_>>(),
                        confidence_interval_lower: result
                            .conf_int_left
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
                        confidence_interval_upper: result
                            .conf_int_right
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
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
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::Regression))?;
            linear_fit(
                "prais",
                &y,
                &x,
                result.betas.iter().copied().collect::<Vec<_>>(),
                RegressionStatistics::Prais {
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.iter().copied().collect::<Vec<_>>(),
                        statistic_values: result.tvalues.iter().copied().collect::<Vec<_>>(),
                        p_values: result.pvalues.iter().copied().collect::<Vec<_>>(),
                        confidence_interval_lower: result
                            .conf_int_left
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
                        confidence_interval_upper: result
                            .conf_int_right
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
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
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::Regression))?;
            let coefficients = result.betas.iter().copied().collect::<Vec<_>>();
            let fitted = (&x * &result.betas)
                .iter()
                .map(|&value| 1.0 / (1.0 + (-value).exp()))
                .collect::<Vec<_>>();
            let adjusted_pseudo_r2 =
                1.0 - (result.log_likelihood - coefficients.len() as f64) / result.ll_null;
            Ok(RegressionFit {
                constant: true,
                family: "logit",
                residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
                fitted,
                coefficients,
                statistics: RegressionStatistics::Binary {
                    link: BinaryRegressionLink::Logit,
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.iter().copied().collect::<Vec<_>>(),
                        statistic_values: result.zvalues.iter().copied().collect::<Vec<_>>(),
                        p_values: result.pvalues.iter().copied().collect::<Vec<_>>(),
                        confidence_interval_lower: result
                            .conf_int_left
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
                        confidence_interval_upper: result
                            .conf_int_right
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
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
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::Regression))?;
            let coefficients = result.betas.iter().copied().collect::<Vec<_>>();
            let normal = Normal::new(0.0, 1.0)
                .map_err(|_| computation_failed(SciOperationCode::Regression))?;
            let fitted = (&x * &result.betas)
                .iter()
                .map(|&value| normal.cdf(value))
                .collect::<Vec<_>>();
            let adjusted_pseudo_r2 =
                1.0 - (result.log_likelihood - coefficients.len() as f64) / result.ll_null;
            Ok(RegressionFit {
                constant: true,
                family: "probit",
                residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
                fitted,
                coefficients,
                statistics: RegressionStatistics::Binary {
                    link: BinaryRegressionLink::Probit,
                    coefficients: RegressionCoefficientStatistics {
                        covariance: covariance_rows(&result.cov_beta),
                        standard_errors: result.stds.iter().copied().collect::<Vec<_>>(),
                        statistic_values: result.zvalues.iter().copied().collect::<Vec<_>>(),
                        p_values: result.pvalues.iter().copied().collect::<Vec<_>>(),
                        confidence_interval_lower: result
                            .conf_int_left
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
                        confidence_interval_upper: result
                            .conf_int_right
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
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

pub fn fit_ols(
    response: Vec<f64>,
    predictors: &[Vec<f64>],
    config: OlsOptions,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    if response.len() <= predictors.len() + usize::from(config.constant)
        || response
            .iter()
            .chain(predictors.iter().flatten())
            .any(|value| !value.is_finite())
    {
        return Err(invalid_input(
            SciOperationCode::Regression,
            SciInputViolation::ParameterOutOfRange,
        ));
    }
    let y = Col::from_iter(response);
    let x = design_matrix(
        predictors,
        y.nrows(),
        config.constant,
        SciOperationCode::Regression,
    )?;
    fit_ols_design(&y, &x, &config, metadata)
}

fn fit_ols_design(
    y: &Col<f64>,
    x: &Mat<f64>,
    config: &OlsOptions,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    let result = OLS {
        endog: y.clone(),
        exog: x.clone(),
        config: config.clone(),
    }
    .fit()
    .map_err(|_| computation_failed(SciOperationCode::Regression))?;
    Ok(RegressionFit {
        constant: config.constant,
        family: "ols",
        coefficients: result.betas.iter().copied().collect::<Vec<_>>(),
        fitted: result.fitted.iter().copied().collect::<Vec<_>>(),
        residuals: result.residuals.iter().copied().collect::<Vec<_>>(),
        statistics: RegressionStatistics::Linear {
            coefficients: RegressionCoefficientStatistics {
                covariance: covariance_rows(&result.cov_beta),
                standard_errors: result.stds.iter().copied().collect::<Vec<_>>(),
                statistic_values: result.tvalues.iter().copied().collect::<Vec<_>>(),
                p_values: result.pvalues.iter().copied().collect::<Vec<_>>(),
                confidence_interval_lower: result.conf_int_left.iter().copied().collect::<Vec<_>>(),
                confidence_interval_upper: result
                    .conf_int_right
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
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
    })
}

fn linear_fit(
    family: &'static str,
    y: &Col<f64>,
    x: &Mat<f64>,
    coefficients: Vec<f64>,
    statistics: RegressionStatistics,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    let fitted = (x * faer::ColRef::from_slice(&coefficients))
        .iter()
        .copied()
        .collect::<Vec<_>>();
    Ok(RegressionFit {
        constant: true,
        family,
        residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
        fitted,
        coefficients,
        statistics,
        metadata,
    })
}

fn design_condition_number(design: &Mat<f64>) -> f64 {
    matrix_rank(design.as_ref()).map_or(f64::INFINITY, |(_, condition)| condition)
}

fn design_matrix(
    predictors: &[Vec<f64>],
    observations: usize,
    constant: bool,
    operation: SciOperationCode,
) -> Result<Mat<f64>, SciError> {
    if predictors.is_empty() {
        return Err(invalid_input(operation, SciInputViolation::EmptyInput));
    }
    if predictors.iter().any(|values| values.len() != observations) {
        return Err(invalid_input(operation, SciInputViolation::ShapeMismatch));
    }
    let columns = predictors.len() + usize::from(constant);
    Ok(Mat::from_fn(observations, columns, |row, column| {
        if constant && column == 0 {
            1.0
        } else {
            predictors[column - usize::from(constant)][row]
        }
    }))
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
) -> Result<serde_json::Value, SciError> {
    let observations = response.len();
    if [exogenous.len(), endogenous.len(), instruments.len()]
        .into_iter()
        .any(|len| len != observations)
    {
        return Err(invalid_input(
            SciOperationCode::InstrumentalVariables,
            SciInputViolation::ShapeMismatch,
        ));
    }
    let column = |values: Vec<f64>| Mat::from_fn(observations, 1, |row, _| values[row]);
    match kind {
        InstrumentalVariableKind::TwoStageLeastSquares => {
            let result = IV2SLS {
                endog: Col::from_iter(response),
                exog: column(exogenous),
                endog_reg: column(endogenous),
                instruments: column(instruments),
                config: IV2SLSConfig {
                    constant: true,
                    cov_type: "nonrobust".into(),
                    cov_params: None,
                    small: false,
                },
                endog_names: None,
                z_var_names: None,
            }
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))?;
            Ok(serde_json::json!({
                "family": "iv_2sls",
                "coefficients": result.betas.iter().copied().collect::<Vec<_>>(),
                "standardErrors": result.stds.iter().copied().collect::<Vec<_>>(),
                "pValues": result.pvalues.iter().copied().collect::<Vec<_>>(),
                "r2": result.r2,
                "adjustedR2": result.r2_adjusted,
                "firstStageMinEigenvalue": result.first_stage_summary.min_eigenvalue,
            }))
        }
        InstrumentalVariableKind::LimitedInformationMaximumLikelihood => {
            let result = IVLIML {
                endog: Col::from_iter(response),
                exog: column(exogenous),
                endog_reg: column(endogenous),
                instruments: column(instruments),
                config: IVLIMLConfig {
                    constant: true,
                    cov_type: "nonrobust".into(),
                    cov_params: None,
                    small: false,
                },
                endog_names: None,
                z_var_names: None,
            }
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))?;
            Ok(serde_json::json!({
                "family": "iv_liml",
                "coefficients": result.betas.iter().copied().collect::<Vec<_>>(),
                "standardErrors": result.stds.iter().copied().collect::<Vec<_>>(),
                "pValues": result.pvalues.iter().copied().collect::<Vec<_>>(),
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
) -> Result<serde_json::Value, SciError> {
    let observations = response.len();
    if entity.len() != observations || time.len() != observations {
        return Err(invalid_input(
            SciOperationCode::Panel,
            SciInputViolation::ShapeMismatch,
        ));
    }
    let mut predictors = predictors;
    let is_did = treatment.is_some();
    if let Some(treatment) = treatment {
        if treatment.len() != observations {
            return Err(invalid_input(
                SciOperationCode::Panel,
                SciInputViolation::ShapeMismatch,
            ));
        }
        predictors.push(treatment);
    }
    let exog = design_matrix(&predictors, observations, false, SciOperationCode::Panel)?;
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
        &Col::from_iter(response),
        &exog,
        &ids(entity),
        &ids(time),
        true,
        "cluster",
        None,
    )
    .map_err(|_| computation_failed(SciOperationCode::Panel))?;
    Ok(serde_json::json!({
        "family": if is_did { "panel_did_twfe" } else { "panel_fe_twoway" },
        "coefficients": result.betas.iter().copied().collect::<Vec<_>>(),
        "standardErrors": result.stds.iter().copied().collect::<Vec<_>>(),
        "pValues": result.pvalues.iter().copied().collect::<Vec<_>>(),
        "r2": result.r2,
        "adjustedR2": result.r2_adjusted,
        "observations": result.num_observation,
        "entities": result.num_entities,
        "timePeriods": result.num_time_periods,
    }))
}

fn covariance_rows(covariance: &Mat<f64>) -> Vec<Vec<f64>> {
    covariance
        .row_iter()
        .map(|row| row.iter().copied().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::report::regression_report;
    use super::*;
    use crate::time_series::augmented_dickey_fuller;
    use yss_sci_contract::{MissingValuePolicy, StatisticalObservationMetadata};
    use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};

    fn regression_metadata(observations: usize) -> StatisticalObservationMetadata {
        StatisticalObservationMetadata {
            original_observation_count: observations,
            used_observation_count: observations,
            dropped_null_count: 0,
            dropped_nan_count: 0,
            missing_value_policy: MissingValuePolicy::Listwise,
        }
    }

    #[test]
    fn design_and_covariance_keep_axis_order_at_report_boundary() {
        let design = design_matrix(
            &[vec![2.0, 3.0], vec![5.0, 7.0]],
            2,
            true,
            SciOperationCode::Regression,
        )
        .unwrap();
        assert_eq!(design, faer::mat![[1.0, 2.0, 5.0], [1.0, 3.0, 7.0]]);
        let values = Mat::from_fn(2, 3, |row, column| (row * 3 + column) as f64);
        assert_eq!(
            covariance_rows(&values),
            vec![vec![0.0, 1.0, 2.0], vec![3.0, 4.0, 5.0]],
        );
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

        let report = regression_report(&fit).expect("regression report must serialize");

        assert_eq!(
            report["coefficients"]
                .as_array()
                .unwrap()
                .iter()
                .map(|coefficient| coefficient["coef"].as_f64().unwrap())
                .collect::<Vec<_>>(),
            fit.coefficients
        );
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
            let report = regression_report(&fit).expect("binary report must serialize");
            let basic = report["model_basic_info"]
                .as_object()
                .expect("binary report must keep the canonical model info object");

            for field in [
                "num_observation",
                "pseudo_r2",
                "adjusted_pseudo_r2",
                "lr_chi2",
                "prob_lr_chi2",
                "df_model",
                "df_residual",
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
            assert_ne!(basic["pseudo_r2"], serde_json::json!(0.0));
            assert_ne!(basic["lr_chi2"], serde_json::json!(0.0));
            assert_ne!(basic["prob_lr_chi2"], serde_json::json!(1.0));
            assert!(!basic.contains_key("f_statistic"));
            assert!(!basic.contains_key("wald_chi2"));
            assert!(!basic.contains_key("r_squared"));

            for field in ["log_likelihood", "lr_chi2", "prob_lr_chi2", "aic", "bic"] {
                assert!(basic[field].as_f64().is_some(), "missing {field}");
            }

            let statistics = report["model_statistics"]
                .as_object()
                .expect("binary report must expose structured model statistics");
            assert_eq!(statistics["kind"], "binary");
            assert_eq!(statistics["link"], expected_link);
            assert_eq!(statistics["pseudoR2"], basic["pseudo_r2"]);
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

        let report = regression_report(&fit).expect("Prais report must serialize");
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

        assert_eq!(
            error,
            SciError::InvalidInput {
                operation: SciOperationCode::Adf,
                violation: SciInputViolation::ParameterOutOfRange,
            }
        );
    }
}
