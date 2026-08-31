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
