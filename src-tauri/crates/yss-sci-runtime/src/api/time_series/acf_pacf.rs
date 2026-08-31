//! ACF/PACF application API and Rust backend orchestration.

use crate::backends::rust;
use serde::{Deserialize, Serialize};
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};

#[derive(Debug, Clone, Deserialize)]
pub struct AcfPacfInput {
    /// Residual or time-series values.
    pub residuals: Vec<f64>,
    /// Requested maximum lag.
    pub max_lag: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AcfPacfOutput {
    /// ACF at lag 0 through `max_lag`; lag 0 is always 1.0.
    pub acf: Vec<f64>,
    /// PACF at lag 1 through `max_lag`.
    pub pacf: Vec<f64>,
    /// Observation count, used by the frontend confidence interval.
    pub n: usize,
}

/// Computes ACF/PACF through the Rust scientific backend.
pub fn compute_acf_pacf(input: AcfPacfInput) -> Result<AcfPacfOutput, SciError> {
    let max_lag = command_max_lag(&input)?;
    rust::time_series::acf_pacf::compute_at_lag(&input.residuals, max_lag)
}

fn command_max_lag(input: &AcfPacfInput) -> Result<usize, SciError> {
    let n = input.residuals.len();
    if n < 4 {
        return Err(SciError::InvalidInput {
            operation: SciOperationCode::AcfPacf,
            violation: SciInputViolation::EmptyInput,
        });
    }
    Ok(input.max_lag.min(n / 2 - 1).clamp(1, 40))
}
