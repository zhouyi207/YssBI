//! Binary-response design preparation and projection of computed fits.
use super::{Logit, Probit};
use crate::error::computation_failed;
use crate::regression::design::{covariance_rows, design_condition_number};
use statrs::distribution::{ContinuousCDF, Normal};
use yss_sci_contract::regression::fit::{
    BinaryRegressionLink, BinaryRegressionStatistics, RegressionCoefficientStatistics,
    RegressionFit, RegressionStatistics,
};
use yss_sci_contract::{SciError, SciOperationCode, StatisticalObservationMetadata};
use yss_sci_linalg::{Col, Mat};

pub fn fit_binary(
    link: BinaryRegressionLink,
    response: Vec<f64>,
    predictors: &[Vec<f64>],
    options: yss_sci_contract::regression::discrete::BinaryOptions,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    let x = crate::regression::design::design_matrix(
        predictors,
        response.len(),
        options.constant,
        SciOperationCode::Regression,
    )?;
    let y = Col::from_iter(response);
    match link {
        BinaryRegressionLink::Logit => fit_logit_design(y, x, metadata, options),
        BinaryRegressionLink::Probit => fit_probit_design(y, x, metadata, options),
    }
}

pub fn predict_binary(
    link: BinaryRegressionLink,
    coefficients: &[f64],
    predictors: &[Vec<f64>],
    constant: bool,
) -> Result<Vec<f64>, SciError> {
    let n = predictors.first().map_or(0, Vec::len);
    let x = crate::regression::design::design_matrix(
        predictors,
        n,
        constant,
        SciOperationCode::Regression,
    )?;
    if x.ncols() != coefficients.len() || coefficients.iter().any(|v| !v.is_finite()) {
        return Err(crate::error::invalid_input(
            SciOperationCode::Regression,
            yss_sci_contract::SciInputViolation::ShapeMismatch,
        ));
    }
    let normal =
        Normal::new(0.0, 1.0).map_err(|_| computation_failed(SciOperationCode::Regression))?;
    Ok((&x * &Col::from_iter(coefficients.iter().copied()))
        .iter()
        .map(|&v| match link {
            BinaryRegressionLink::Logit if v >= 0.0 => 1.0 / (1.0 + (-v).exp()),
            BinaryRegressionLink::Logit => v.exp() / (1.0 + v.exp()),
            BinaryRegressionLink::Probit => normal.cdf(v),
        })
        .collect())
}

pub(crate) fn fit_logit_design(
    y: Col<f64>,
    x: Mat<f64>,
    metadata: StatisticalObservationMetadata,
    config: yss_sci_contract::regression::discrete::BinaryOptions,
) -> Result<RegressionFit, SciError> {
    let constant = config.constant;
    let result = Logit {
        endog: y.clone(),
        exog: x.clone(),
        config,
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
        constant,
        family: "logit".into(),
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
                confidence_interval_lower: result.conf_int_left.iter().copied().collect::<Vec<_>>(),
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

pub(crate) fn fit_probit_design(
    y: Col<f64>,
    x: Mat<f64>,
    metadata: StatisticalObservationMetadata,
    config: yss_sci_contract::regression::discrete::BinaryOptions,
) -> Result<RegressionFit, SciError> {
    let constant = config.constant;
    let result = Probit {
        endog: y.clone(),
        exog: x.clone(),
        config,
    }
    .fit()
    .map_err(|_| computation_failed(SciOperationCode::Regression))?;
    let coefficients = result.betas.iter().copied().collect::<Vec<_>>();
    let normal =
        Normal::new(0.0, 1.0).map_err(|_| computation_failed(SciOperationCode::Regression))?;
    let fitted = (&x * &result.betas)
        .iter()
        .map(|&value| normal.cdf(value))
        .collect::<Vec<_>>();
    let adjusted_pseudo_r2 =
        1.0 - (result.log_likelihood - coefficients.len() as f64) / result.ll_null;
    Ok(RegressionFit {
        constant,
        family: "probit".into(),
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
                confidence_interval_lower: result.conf_int_left.iter().copied().collect::<Vec<_>>(),
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
