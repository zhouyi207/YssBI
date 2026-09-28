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
    Ok(result)
}

#[cfg(test)]
mod tests;
