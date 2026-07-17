//! Serial-correlation test application API and engine orchestration.
//!
//! Covers Durbin-Watson, Ljung-Box Q, and optional Breusch-Godfrey LM tests.

use serde::{Deserialize, Serialize};

use crate::sci::backends::{julia, rust};
use crate::sci::engine::{SciContext, SciEngine};
use crate::sci::error::SciError;

#[derive(Debug, Clone, Deserialize)]
pub struct SerialTestsInput {
    /// Residual series.
    pub residuals: Vec<f64>,
    /// Requested lag count for BG/Q tests.
    pub lags: usize,
    /// Row-major regression design matrix. Required for BG; absent skips BG.
    #[serde(default)]
    pub exog: Option<Vec<Vec<f64>>>,
    /// BG test mode: true = nomiss0; false = drop the first p observations.
    #[serde(default = "default_bg_nomiss0", alias = "bgNomiss0")]
    pub bg_nomiss0: bool,
}

fn default_bg_nomiss0() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SerialTestWithLag {
    pub stat: f64,
    pub p_value: f64,
    pub lags: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DurbinWatsonResult {
    pub d: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SerialTestsOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<SerialTestWithLag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<SerialTestWithLag>,
    pub dw: DurbinWatsonResult,
}

pub fn compute_serial_tests(
    context: &SciContext<'_>,
    input: SerialTestsInput,
) -> Result<SerialTestsOutput, SciError> {
    match context.engine {
        SciEngine::Rust => compute_with_rust(&input),
        SciEngine::Julia => compute_with_julia(context, input),
        SciEngine::JuliaWithRustFallback => {
            let rust_input = input.clone();
            compute_with_julia(context, input).or_else(|_| compute_with_rust(&rust_input))
        }
    }
}

fn compute_with_rust(input: &SerialTestsInput) -> Result<SerialTestsOutput, SciError> {
    let lags = normalized_lags(input)?;
    Ok(rust::time_series::serial_tests::compute(
        &input.residuals,
        input.exog.as_deref(),
        lags,
        input.bg_nomiss0,
    ))
}

fn compute_with_julia(
    context: &SciContext<'_>,
    input: SerialTestsInput,
) -> Result<SerialTestsOutput, SciError> {
    let lags = normalized_lags(&input)?;
    julia::time_series::serial_tests::compute(context, input, lags)
}

fn normalized_lags(input: &SerialTestsInput) -> Result<usize, SciError> {
    let n = input.residuals.len();
    if n < 4 {
        return Err(SciError::invalid_input("序列相关检验: 至少需要 4 个观测值"));
    }
    Ok(input.lags.min(n / 2 - 1).min(40).max(1))
}
