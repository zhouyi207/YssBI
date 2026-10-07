//! Linear regression entry points with admission and execution-control checks.
pub mod prais;
use crate::error::{invalid, map_sci_error};
use yss_sci_contract::execution::{
    ScientificComputationError, ScientificExecutionControl, ScientificInputViolation,
};
use yss_sci_contract::regression::OlsOptions;
use yss_sci_contract::regression::fit::RegressionFit;
use yss_sci_contract::{SciError, StatisticalObservationMetadata};

pub fn linear_regression(
    request: yss_sci_contract::regression::linear::LinearRegressionRequest,
    control: &ScientificExecutionControl,
) -> Result<yss_sci_contract::regression::linear::LinearRegressionResult, ScientificComputationError>
{
    use yss_sci_contract::regression::linear::LinearRegressionResult;
    control.check()?;
    let observations = request.response.len();
    if request.predictors.is_empty()
        || observations <= request.predictors.len() + usize::from(request.options.constant)
    {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    if request
        .predictors
        .iter()
        .any(|column| column.len() != observations)
    {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    for (index, value) in request
        .response
        .iter()
        .chain(request.predictors.iter().flatten())
        .enumerate()
    {
        if index % 1024 == 0 {
            control.check()?;
        }
        if !value.is_finite() {
            return Err(invalid(ScientificInputViolation::NonFiniteInput));
        }
    }
    validate_ols_covariance(&request.options.covariance, observations)?;
    let metadata = yss_sci_contract::StatisticalObservationMetadata {
        original_observation_count: observations,
        used_observation_count: observations,
        dropped_null_count: 0,
        dropped_nan_count: 0,
        missing_value_policy: yss_sci_contract::MissingValuePolicy::Reject,
    };
    let weights = match &request.method {
        yss_sci_contract::regression::linear::LinearRegressionMethod::Wls { weights } => {
            Some(weights.clone())
        }
        _ => None,
    };
    let constant = request.options.constant;
    control.check()?;
    let fit = yss_sci::regression::linear::fit::fit_linear_regression(
        request.response,
        &request.predictors,
        request.options,
        request.method,
        metadata,
    )
    .map_err(map_sci_error)?;
    control.check()?;
    let report = crate::regression::report::linear_regression_report(&fit)
        .map_err(|_| ScientificComputationError::ComputationFailed)?;
    let mut design = request.predictors;
    if constant {
        design.insert(0, vec![1.0; observations]);
    }
    control.check()?;
    Ok(LinearRegressionResult {
        weights,
        constant,
        coefficients: fit.coefficients,
        fitted: fit.fitted,
        residuals: fit.residuals,
        design,
        report,
    })
}

fn validate_ols_covariance(
    covariance: &yss_sci_contract::regression::OlsCovariance,
    observations: usize,
) -> Result<(), ScientificComputationError> {
    use yss_sci_contract::regression::OlsCovariance;
    let valid = match covariance {
        OlsCovariance::NonRobust
        | OlsCovariance::Hc0
        | OlsCovariance::Hc1
        | OlsCovariance::Hc2
        | OlsCovariance::Hc3 => true,
        OlsCovariance::FixedScale { scale } => scale.is_finite() && *scale > 0.0,
        OlsCovariance::Cluster { cluster_id, .. } => cluster_id.len() == observations,
        OlsCovariance::Hac { kernel, bandwidth } => {
            bandwidth.is_none_or(|value| value > 0)
                && matches!(
                    kernel.as_str(),
                    "bartlett" | "parzen" | "quadratic spectral"
                )
        }
        OlsCovariance::Newey { lag } => lag.is_none_or(|value| value >= 0),
    };
    if valid {
        Ok(())
    } else {
        Err(invalid(ScientificInputViolation::ParameterOutOfRange))
    }
}

pub fn fit_ols(
    response: Vec<f64>,
    predictors: &[Vec<f64>],
    config: OlsOptions,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    yss_sci::regression::linear::fit::fit_ols(response, predictors, config, metadata)
}

#[cfg(test)]
mod tests;
