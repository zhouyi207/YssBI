//! ACF/PACF application API and engine orchestration.

use crate::sci::backends::{julia, rust};
use crate::sci::engine::{SciContext, SciEngine};
use crate::sci::error::SciError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct AcfPacfInput {
    /// Residual or time-series values.
    pub residuals: Vec<f64>,
    /// Requested maximum lag.
    #[serde(alias = "maxLag")]
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

/// Computes ACF/PACF through the selected scientific engine.
pub fn compute_acf_pacf(
    context: &SciContext<'_>,
    input: AcfPacfInput,
) -> Result<AcfPacfOutput, SciError> {
    match context.engine {
        SciEngine::Rust => compute_with_rust(&input),
        SciEngine::Julia => compute_with_julia(context, input),
        SciEngine::JuliaWithRustFallback => {
            let rust_input = input.clone();
            compute_with_julia(context, input).or_else(|_| compute_with_rust(&rust_input))
        }
    }
}

fn compute_with_rust(input: &AcfPacfInput) -> Result<AcfPacfOutput, SciError> {
    let max_lag = command_max_lag(input)?;
    rust::time_series::acf_pacf::compute_at_lag(&input.residuals, max_lag)
}

fn compute_with_julia(
    context: &SciContext<'_>,
    input: AcfPacfInput,
) -> Result<AcfPacfOutput, SciError> {
    let max_lag = command_max_lag(&input)?;
    julia::time_series::acf_pacf::compute(context, input, max_lag)
}

fn command_max_lag(input: &AcfPacfInput) -> Result<usize, SciError> {
    let n = input.residuals.len();
    if n < 4 {
        return Err(SciError::invalid_input("ACF/PACF: 至少需要 4 个观测值"));
    }
    Ok(input.max_lag.min(n / 2 - 1).min(40).max(1))
}
