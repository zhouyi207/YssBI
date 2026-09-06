//! `yss-sci-runtime` implementation of Execution's live scientific backend port.

use std::time::Instant;

use yss_execution::ports::scientific::{
    AcfPacfRequest, AcfPacfResult, BackendExecutionControl, ScientificBackend,
    ScientificBackendError, ScientificInputViolation,
};
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};
use yss_sci_runtime::api::time_series::acf_pacf::{
    AcfPacfInput, compute_acf_pacf as compute_acf_pacf_api,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct SciRuntimeBackend;

impl SciRuntimeBackend {
    pub const fn new() -> Self {
        Self
    }
}

impl ScientificBackend for SciRuntimeBackend {
    fn ols(
        &self,
        request: yss_execution::ports::scientific::OlsRequest,
        control: &BackendExecutionControl,
    ) -> Result<yss_execution::ports::scientific::OlsResult, ScientificBackendError> {
        use yss_execution::ports::scientific::{OlsCovariance, OlsResult};
        use yss_sci_runtime::models::regression::{OLSConfigure, OLSCovarianceConfig};
        admit(control)?;
        let observations = request.response.len();
        if request.predictors.is_empty()
            || observations <= request.predictors.len() + usize::from(request.constant)
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
        let (cov_type, cov_config) = match request.covariance {
            OlsCovariance::NonRobust => ("nonrobust", None),
            OlsCovariance::Hc0 => ("HC0", None),
            OlsCovariance::Hc1 => ("HC1", None),
            OlsCovariance::Hc2 => ("HC2", None),
            OlsCovariance::Hc3 => ("HC3", None),
            OlsCovariance::FixedScale { scale } if scale.is_finite() && scale > 0.0 => (
                "fixed scale",
                Some(OLSCovarianceConfig::FixedScale { scale }),
            ),
            OlsCovariance::Hac { kernel, bandwidth }
                if bandwidth > 0
                    && matches!(
                        kernel.as_ref(),
                        "bartlett" | "parzen" | "quadratic spectral"
                    ) =>
            {
                let bandwidth = i64::try_from(bandwidth)
                    .map_err(|_| invalid(ScientificInputViolation::ParameterOutOfRange))?;
                (
                    "HAC",
                    Some(OLSCovarianceConfig::HAC {
                        kernel: kernel.into_string(),
                        bandwidth: Some(bandwidth),
                    }),
                )
            }
            OlsCovariance::Newey { lag } => {
                let lag = i64::try_from(lag)
                    .map_err(|_| invalid(ScientificInputViolation::ParameterOutOfRange))?;
                ("newey", Some(OLSCovarianceConfig::Newey { lag: Some(lag) }))
            }
            _ => return Err(invalid(ScientificInputViolation::ParameterOutOfRange)),
        };
        let metadata = yss_sci_contract::StatisticalObservationMetadata {
            original_observation_count: observations,
            used_observation_count: observations,
            dropped_null_count: 0,
            dropped_nan_count: 0,
            missing_value_policy: yss_sci_contract::MissingValuePolicy::Reject,
        };
        let fit = yss_sci_runtime::api::node_statistics::fit_ols(
            request.response,
            request.predictors,
            OLSConfigure {
                constant: request.constant,
                cov_type: cov_type.into(),
                cov_config,
            },
            metadata,
        )
        .map_err(|_| ScientificBackendError::ComputationFailed)?;
        let report = yss_sci_runtime::api::node_statistics::regression_report(&fit)
            .map_err(|_| ScientificBackendError::ComputationFailed)?;
        admit(control)?;
        Ok(OlsResult {
            coefficients: fit.coefficients,
            fitted: fit.fitted,
            residuals: fit.residuals,
            report,
        })
    }
    fn acf_pacf(
        &self,
        request: AcfPacfRequest,
        control: &BackendExecutionControl,
    ) -> Result<AcfPacfResult, ScientificBackendError> {
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
}

fn admit(control: &BackendExecutionControl) -> Result<(), ScientificBackendError> {
    if control.cancellation.is_cancelled() {
        return Err(ScientificBackendError::Cancelled);
    }
    if control.deadline <= Instant::now() {
        return Err(ScientificBackendError::DeadlineExceeded);
    }
    Ok(())
}

fn validate_acf_pacf_request(request: &AcfPacfRequest) -> Result<(), ScientificBackendError> {
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

const fn invalid(violation: ScientificInputViolation) -> ScientificBackendError {
    ScientificBackendError::InvalidInput { violation }
}

fn map_sci_error(error: SciError) -> ScientificBackendError {
    match error {
        SciError::InvalidInput {
            operation,
            violation,
        } => {
            if operation != SciOperationCode::AcfPacf {
                return ScientificBackendError::ComputationFailed;
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
        SciError::ComputationFailed { .. } => ScientificBackendError::ComputationFailed,
    }
}

#[cfg(test)]
mod tests;
