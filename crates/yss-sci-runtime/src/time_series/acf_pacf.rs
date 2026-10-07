//! ACF/PACF admission and report lag budget.
use crate::error::invalid;
use yss_sci_contract::execution::{
    ScientificComputationError, ScientificExecutionControl, ScientificInputViolation,
};
use yss_sci_contract::time_series::acf_pacf::{AcfPacfRequest, AcfPacfResult};

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
    let result =
        yss_sci::time_series::acf_pacf::compute_acf_pacf(&request.values, max_lag, control)?;
    control.check()?;
    validate_result(&result)?;
    Ok(result)
}

fn validate_result(result: &AcfPacfResult) -> Result<(), ScientificComputationError> {
    if result.ci_half_width.is_finite()
        && result.ci_half_width > 0.0
        && result
            .acf
            .iter()
            .chain(&result.pacf)
            .all(|value| value.is_finite())
    {
        Ok(())
    } else {
        Err(ScientificComputationError::ComputationFailed)
    }
}

#[cfg(test)]
mod tests;
