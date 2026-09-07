use crate::error::{computation_failed, invalid_input};
use serde::Serialize;
use yss_sci_contract::regression::OlsOptions;
pub mod report;
pub mod types;
use crate::regression::types::{
    BinaryRegressionLink, BinaryRegressionStatistics, LinearRegressionStatistics,
    PraisRegressionStatistics, RegressionCoefficientStatistics, RegressionStatistics,
};
use ndarray::{Array1, Array2};
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
    let y = Array1::from_vec(response);
    let x = design_matrix(&predictors, y.len(), true, SciOperationCode::Regression)?;
    match kind {
        RegressionKind::Ols => fit_ols_design(&y, &x, &OlsOptions::default(), metadata),
        RegressionKind::Gls => {
            let result = GLS {
                endog: y.clone(),
                exog: x.clone(),
                sigma: Array2::eye(y.len()),
                config: GLSConfig { constant: true },
            }
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::Regression))?;
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
            let weights = weights.ok_or_else(|| {
                invalid_input(SciOperationCode::Regression, SciInputViolation::EmptyInput)
            })?;
            if weights.len() != y.len() {
                return Err(invalid_input(
                    SciOperationCode::Regression,
                    SciInputViolation::ShapeMismatch,
                ));
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
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::Regression))?;
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
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::Regression))?;
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
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::Regression))?;
            let coefficients = result.betas.to_vec();
            let fitted = x
                .dot(&Array1::from_vec(coefficients.clone()))
                .mapv(|value| 1.0 / (1.0 + (-value).exp()))
                .to_vec();
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
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::Regression))?;
            let coefficients = result.betas.to_vec();
            let normal = Normal::new(0.0, 1.0)
                .map_err(|_| computation_failed(SciOperationCode::Regression))?;
            let fitted = x
                .dot(&Array1::from_vec(coefficients.clone()))
                .mapv(|value| normal.cdf(value))
                .to_vec();
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

pub fn fit_ols(
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
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
    let y = Array1::from_vec(response);
    let x = design_matrix(
        &predictors,
        y.len(),
        config.constant,
        SciOperationCode::Regression,
    )?;
    fit_ols_design(&y, &x, &config, metadata)
}

fn fit_ols_design(
    y: &Array1<f64>,
    x: &Array2<f64>,
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
        coefficients: result.betas.to_vec(),
        fitted: result.fitted.to_vec(),
        residuals: result.residuals.to_vec(),
        statistics: RegressionStatistics::Linear {
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
    })
}

fn linear_fit(
    family: &'static str,
    y: &Array1<f64>,
    x: &Array2<f64>,
    coefficients: Vec<f64>,
    statistics: RegressionStatistics,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    let fitted = x.dot(&Array1::from_vec(coefficients.clone())).to_vec();
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

fn design_condition_number(design: &Array2<f64>) -> f64 {
    matrix_rank(design.view()).map_or(f64::INFINITY, |(_, condition)| condition)
}

fn design_matrix(
    predictors: &[Vec<f64>],
    observations: usize,
    constant: bool,
    operation: SciOperationCode,
) -> Result<Array2<f64>, SciError> {
    if predictors.is_empty() {
        return Err(invalid_input(operation, SciInputViolation::EmptyInput));
    }
    if predictors.iter().any(|values| values.len() != observations) {
        return Err(invalid_input(operation, SciInputViolation::ShapeMismatch));
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
    Array2::from_shape_vec((observations, columns), values)
        .map_err(|_| computation_failed(operation))
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
    let column = |values: Vec<f64>| {
        Array2::from_shape_vec((observations, 1), values)
            .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))
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
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))?;
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
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))?;
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
        &Array1::from_vec(response),
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

fn covariance_rows(covariance: &Array2<f64>) -> Vec<Vec<f64>> {
    covariance
        .rows()
        .into_iter()
        .map(|row| row.to_vec())
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
