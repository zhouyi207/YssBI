//! Synchronous scientific computation entry points.

use yss_sci_contract::scientific::{
    AcfPacfRequest, AcfPacfResult, ScientificComputationError, ScientificExecutionControl,
    ScientificInputViolation,
};
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};

pub fn linear_regression(
    request: yss_sci_contract::scientific::LinearRegressionRequest,
    control: &ScientificExecutionControl,
) -> Result<yss_sci_contract::scientific::LinearRegressionResult, ScientificComputationError> {
    use yss_sci_contract::scientific::LinearRegressionResult;
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
    let constant = request.options.constant;
    control.check()?;
    let fit = yss_sci::regression::fit::fit_linear_regression(
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
        constant,
        coefficients: fit.coefficients,
        fitted: fit.fitted,
        residuals: fit.residuals,
        design,
        report,
    })
}
pub fn acf_pacf(
    request: AcfPacfRequest,
    control: &ScientificExecutionControl,
) -> Result<AcfPacfResult, ScientificComputationError> {
    control.check()?;
    if request.values.len() < 4 {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    if request.max_lag == 0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    // Report budget, not an algorithmic limit: preserve the existing lag policy.
    let max_lag = request.max_lag.min(request.values.len() / 2 - 1).min(40);
    let result = yss_sci::ts::acf_pacf::compute_acf_pacf(&request.values, max_lag, control)?;
    control.check()?;
    Ok(result)
}

const fn invalid(violation: ScientificInputViolation) -> ScientificComputationError {
    ScientificComputationError::InvalidInput { violation }
}

fn map_sci_error(error: SciError) -> ScientificComputationError {
    match error {
        SciError::InvalidInput {
            operation,
            violation,
        } => {
            if !matches!(
                operation,
                SciOperationCode::AcfPacf | SciOperationCode::Regression
            ) {
                return ScientificComputationError::ComputationFailed;
            }
            invalid(match violation {
                SciInputViolation::EmptyInput => ScientificInputViolation::EmptyInput,
                SciInputViolation::NonFiniteInput => ScientificInputViolation::NonFiniteInput,
                SciInputViolation::ShapeMismatch => ScientificInputViolation::ShapeMismatch,
                SciInputViolation::ParameterOutOfRange => {
                    ScientificInputViolation::ParameterOutOfRange
                }
            })
        }
        SciError::ComputationFailed { .. } => ScientificComputationError::ComputationFailed,
    }
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

#[cfg(test)]
mod tests;
