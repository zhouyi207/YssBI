//! Neutral inputs and reports for univariate forecasting and stationarity.
use crate::regression::models::{IterationOptions, RegressionModelResult};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct ArimaOptions {
    pub p: usize,
    pub d: usize,
    pub q: usize,
    pub seasonal_p: usize,
    pub seasonal_d: usize,
    pub seasonal_q: usize,
    pub period: usize,
    pub constant: bool,
    pub horizon: usize,
    pub confidence: f64,
    pub iteration: IterationOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Seasonality {
    None,
    Additive,
    Multiplicative,
}

#[derive(Debug, Clone, Copy)]
pub struct SmoothingOptions {
    pub trend: bool,
    pub damped: bool,
    pub seasonality: Seasonality,
    pub period: usize,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub phi: f64,
    pub optimize: bool,
    pub horizon: usize,
    pub iteration: IterationOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VolatilityMethod {
    Arch,
    Garch,
    Egarch,
    GjrGarch,
}

#[derive(Debug, Clone, Copy)]
pub struct VolatilityOptions {
    pub method: VolatilityMethod,
    pub p: usize,
    pub q: usize,
    pub constant: bool,
    pub horizon: usize,
    pub simulations: usize,
    pub seed: u64,
    pub iteration: IterationOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParameterEstimate {
    pub term: String,
    pub estimate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForecastResult {
    pub method: String,
    pub observations: usize,
    pub effective_observations: usize,
    pub parameters: Vec<ParameterEstimate>,
    /// Source-row aligned; unestimated initial rows are null.
    pub fitted: Vec<Option<f64>>,
    pub residuals: Vec<Option<f64>>,
    /// Index zero is the first observation after the end of the input.
    pub forecasts: Vec<f64>,
    pub lower: Option<Vec<f64>>,
    pub upper: Option<Vec<f64>>,
    pub innovation_variance: f64,
    pub log_likelihood: Option<f64>,
    pub aic: Option<f64>,
    pub bic: Option<f64>,
    pub iterations: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VolatilityResult {
    pub method: VolatilityMethod,
    pub observations: usize,
    pub parameters: Vec<ParameterEstimate>,
    pub residuals: Vec<f64>,
    pub standardized_residuals: Vec<f64>,
    pub conditional_variances: Vec<f64>,
    pub forecast_variances: Vec<f64>,
    pub log_likelihood: f64,
    pub aic: f64,
    pub bic: f64,
    pub iterations: usize,
    pub simulations: Option<usize>,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Deterministic {
    None,
    Constant,
    Trend,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StationarityResult {
    pub method: String,
    pub observations: usize,
    pub effective_observations: usize,
    pub deterministic: Deterministic,
    pub bandwidth: usize,
    pub statistic: f64,
    pub p_value: f64,
    /// Exact here means evaluated response-surface approximation, not a finite-sample law.
    pub p_value_kind: String,
    pub critical_values: Vec<ParameterEstimate>,
    pub long_run_variance: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct EcmOptions {
    pub lags: usize,
    pub constant: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EcmResult {
    pub observations: usize,
    pub first_short_run_row: usize,
    pub long_run: RegressionModelResult,
    pub short_run: RegressionModelResult,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarkovResult {
    pub observations: usize,
    pub states: usize,
    pub counts: Vec<Vec<usize>>,
    pub transition_probabilities: Vec<Vec<f64>>,
    pub last_state: usize,
    pub forecast_probabilities: Vec<Vec<f64>>,
    pub forecast_states: Vec<usize>,
    pub pseudocount: f64,
}
