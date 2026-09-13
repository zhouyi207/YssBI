//! Synchronous scientific computation entry points.

use std::time::Instant;

use crate::time_series::acf_pacf::{AcfPacfInput, compute_acf_pacf as compute_acf_pacf_api};
use yss_sci_contract::scientific::{
    AcfPacfRequest, AcfPacfResult, ScientificComputationError, ScientificExecutionControl,
    ScientificInputViolation,
};
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};

pub fn ols(
    request: yss_sci_contract::scientific::OlsRequest,
    control: &ScientificExecutionControl,
) -> Result<yss_sci_contract::scientific::OlsResult, ScientificComputationError> {
    use yss_sci_contract::scientific::OlsResult;
    admit(control)?;
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
    if request
        .response
        .iter()
        .chain(request.predictors.iter().flatten())
        .any(|value| !value.is_finite())
    {
        return Err(invalid(ScientificInputViolation::NonFiniteInput));
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
    let fit = crate::regression::fit_ols(
        request.response,
        &request.predictors,
        request.options,
        metadata,
    )
    .map_err(|_| ScientificComputationError::ComputationFailed)?;
    let report = crate::regression::report::ols_report(&fit)
        .map_err(|_| ScientificComputationError::ComputationFailed)?;
    let mut design = request.predictors;
    if constant {
        design.insert(0, vec![1.0; observations]);
    }
    admit(control)?;
    Ok(OlsResult {
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
    admit(control)?;
    validate_acf_pacf_request(&request)?;
    let output = compute_acf_pacf_api(AcfPacfInput {
        residuals: request.values,
        max_lag: request.max_lag,
    })
    .map_err(map_sci_error)?;
    Ok(AcfPacfResult {
        acf: output.acf,
        pacf: output.pacf,
        n: output.n,
    })
}

fn admit(control: &ScientificExecutionControl) -> Result<(), ScientificComputationError> {
    if control.cancellation.is_cancelled() {
        return Err(ScientificComputationError::Cancelled);
    }
    if control.deadline <= Instant::now() {
        return Err(ScientificComputationError::DeadlineExceeded);
    }
    Ok(())
}

fn validate_acf_pacf_request(request: &AcfPacfRequest) -> Result<(), ScientificComputationError> {
    if request.values.len() < 4 {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    if request.values.iter().any(|value| !value.is_finite()) {
        return Err(invalid(ScientificInputViolation::NonFiniteInput));
    }
    if request.max_lag == 0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    Ok(())
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
            if operation != SciOperationCode::AcfPacf {
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
