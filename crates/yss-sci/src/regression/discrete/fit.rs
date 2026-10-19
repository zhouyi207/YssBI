//! Binary-response design preparation and projection of computed fits.
use super::{Logit, Probit};
use crate::error::computation_failed;
use crate::regression::design::{covariance_rows, design_condition_number};
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
    fit_binary_design(link, y, x, metadata, options)
}

fn inverse_link(link: BinaryRegressionLink, value: f64) -> f64 {
    match link {
        // Branching keeps exp from overflowing on either tail.
        BinaryRegressionLink::Logit if value >= 0.0 => 1.0 / (1.0 + (-value).exp()),
        BinaryRegressionLink::Logit => value.exp() / (1.0 + value.exp()),
        BinaryRegressionLink::Probit => crate::distribution::normal::cdf(value),
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
            yss_sci_contract::execution::ScientificInputViolation::ShapeMismatch,
        ));
    }
    Ok((&x * &Col::from_iter(coefficients.iter().copied()))
        .iter()
        .map(|&v| inverse_link(link, v))
        .collect())
}

fn fit_binary_design(
    link: BinaryRegressionLink,
    y: Col<f64>,
    x: Mat<f64>,
    metadata: StatisticalObservationMetadata,
    config: yss_sci_contract::regression::discrete::BinaryOptions,
) -> Result<RegressionFit, SciError> {
    let constant = config.constant;
    let (result, y, x) = match link {
        BinaryRegressionLink::Logit => {
            let model = Logit {
                endog: y,
                exog: x,
                config,
            };
            (model.fit(), model.endog, model.exog)
        }
        BinaryRegressionLink::Probit => {
            let model = Probit {
                endog: y,
                exog: x,
                config,
            };
            (model.fit(), model.endog, model.exog)
        }
    };
    let result = result.map_err(|_| computation_failed(SciOperationCode::Regression))?;
    let (y, x) = (&y, &x);
    let coefficients = result.betas.iter().copied().collect::<Vec<_>>();
    let fitted = (x * &result.betas)
        .iter()
        .map(|&value| inverse_link(link, value))
        .collect::<Vec<_>>();
    let adjusted_pseudo_r2 =
        1.0 - (result.log_likelihood - coefficients.len() as f64) / result.ll_null;
    Ok(RegressionFit {
        parameter_names: (0..x.ncols())
            .map(|i| {
                if i == 0 && constant {
                    "_cons".into()
                } else {
                    format!("x{}", i + usize::from(!constant))
                }
            })
            .collect(),
        response_name: "response".into(),
        design: (0..x.ncols())
            .map(|j| x.col(j).iter().copied().collect())
            .collect(),
        constant,
        family: link.family().into(),
        residuals: y.iter().zip(&fitted).map(|(a, b)| a - b).collect(),
        fitted,
        coefficients,
        statistics: RegressionStatistics::Binary {
            link,
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
                condition_number: design_condition_number(x),
            },
        },
        metadata,
    })
}
