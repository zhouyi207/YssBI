//! Neutral serial-correlation requests and results.
use serde::{Deserialize, Serialize};

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
    #[serde(default = "default_bg_nomiss0")]
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
