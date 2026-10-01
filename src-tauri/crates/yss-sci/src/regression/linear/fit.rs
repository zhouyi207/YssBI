//! Linear-model design preparation and projection of computed fits.
use super::{GLS, GLSConfig, OLS, Prais, PraisConfig, WLS, WLSConfig};
use crate::error::{computation_failed, invalid_input};
use crate::regression::design::{covariance_rows, design_matrix};
use yss_sci_contract::regression::OlsOptions;
use yss_sci_contract::regression::fit::{
    LinearRegressionStatistics, PraisRegressionStatistics, RegressionCoefficientStatistics,
    RegressionFit, RegressionStatistics,
};
use yss_sci_contract::regression::linear::LinearRegressionMethod;
use yss_sci_contract::{
    SciError, SciInputViolation, SciOperationCode, StatisticalObservationMetadata,
};
use yss_sci_linalg::{Col, Mat};

pub fn fit_linear_regression(
    response: Vec<f64>,
    predictors: &[Vec<f64>],
    config: OlsOptions,
    method: LinearRegressionMethod,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    let y = Col::from_iter(response);
    let x = design_matrix(
        predictors,
        y.nrows(),
        config.constant,
        SciOperationCode::Regression,
    )?;
    match &method {
        LinearRegressionMethod::Wls { weights }
            if weights.len() != y.nrows()
                || weights.iter().any(|w| !w.is_finite() || *w <= 0.0) =>
        {
            return Err(invalid_input(
                SciOperationCode::Regression,
                SciInputViolation::ParameterOutOfRange,
            ));
        }
        LinearRegressionMethod::Gls { sigma } => {
            if sigma.len() != y.nrows() || sigma.iter().any(|row| row.len() != y.nrows()) {
                return Err(invalid_input(
                    SciOperationCode::Regression,
                    SciInputViolation::ShapeMismatch,
                ));
            }
            if !matches!(
                config.covariance,
                yss_sci_contract::regression::OlsCovariance::NonRobust
            ) || sigma.iter().enumerate().any(|(i, row)| {
                row.iter()
                    .enumerate()
                    .any(|(j, v)| !v.is_finite() || *v != sigma[j][i])
            }) {
                return Err(invalid_input(
                    SciOperationCode::Regression,
                    SciInputViolation::ParameterOutOfRange,
                ));
            }
        }
        _ => {}
    }
    let mut fit = match method {
        LinearRegressionMethod::Ols => fit_ols_design(&y, &x, &config, metadata),
        LinearRegressionMethod::Gls { sigma } => {
            let result = GLS {
                endog: y.clone(),
                exog: x.clone(),
                sigma: Mat::from_fn(y.nrows(), y.nrows(), |i, j| sigma[i][j]),
                config: GLSConfig {
                    constant: config.constant,
                },
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
        LinearRegressionMethod::Wls { weights } => {
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
                    constant: config.constant,
                    covariance: config.covariance.clone(),
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
    }?;
    fit.constant = config.constant;
    fit.parameter_names = (0..fit.coefficients.len())
        .map(|i| {
            if config.constant && i == 0 {
                "_cons".into()
            } else {
                format!("x{}", i + usize::from(!config.constant))
            }
        })
        .collect();
    Ok(fit)
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

pub(crate) fn fit_ols_design(
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
        parameter_names: (0..x.ncols())
            .map(|i| {
                if i == 0 && config.constant {
                    "_cons".into()
                } else {
                    format!("x{}", i + usize::from(!config.constant))
                }
            })
            .collect(),
        response_name: "response".into(),
        design: (0..x.ncols())
            .map(|j| x.col(j).iter().copied().collect())
            .collect(),
        constant: config.constant,
        family: "ols".into(),
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
    let fitted = (x * yss_sci_linalg::ColRef::from_slice(&coefficients))
        .iter()
        .copied()
        .collect::<Vec<_>>();
    Ok(RegressionFit {
        parameter_names: (0..x.ncols())
            .map(|i| {
                if i == 0 && x.col(0).iter().all(|v| *v == 1.0) {
                    "_cons".into()
                } else {
                    format!("x{}", i + usize::from(!x.col(0).iter().all(|v| *v == 1.0)))
                }
            })
            .collect(),
        response_name: "response".into(),
        design: (0..x.ncols())
            .map(|j| x.col(j).iter().copied().collect())
            .collect(),
        constant: true,
        family: family.into(),
        residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
        fitted,
        coefficients,
        statistics,
        metadata,
    })
}

pub fn fit_prais(
    response: Vec<f64>,
    predictors: &[Vec<f64>],
    config: PraisConfig,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    let x = design_matrix(
        predictors,
        response.len(),
        config.constant,
        SciOperationCode::Regression,
    )?;
    fit_prais_design(Col::from_iter(response), x, metadata, config)
}

pub(crate) fn fit_prais_design(
    y: Col<f64>,
    x: Mat<f64>,
    metadata: StatisticalObservationMetadata,
    config: PraisConfig,
) -> Result<RegressionFit, SciError> {
    let constant = config.constant;
    let transform = format!("{:?}", config.transform);
    let result = Prais {
        endog: y.clone(),
        exog: x.clone(),
        config,
    }
    .fit()
    .map_err(|_| computation_failed(SciOperationCode::Regression))?;
    let mut fit = linear_fit(
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
                confidence_interval_lower: result.conf_int_left.iter().copied().collect::<Vec<_>>(),
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
                iteration_log: result.iteration_log,
                rho_history: result.rho_history,
                transform,
                durbin_watson_original: result.dw_original,
                durbin_watson_transformed: result.dw_transformed,
                iterations: result.iterations,
            },
        },
        metadata,
    )?;
    fit.constant = constant;
    fit.parameter_names = (0..fit.coefficients.len())
        .map(|i| {
            if constant && i == 0 {
                "_cons".into()
            } else {
                format!("x{}", i + usize::from(!constant))
            }
        })
        .collect();
    Ok(fit)
}
