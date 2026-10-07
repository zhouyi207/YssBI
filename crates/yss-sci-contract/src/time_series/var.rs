//! VAR fit facts; postestimation results are computed independently.
use super::fit::MultivariateStatistics;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarFit {
    pub var_names: Vec<String>,
    pub lags: Vec<usize>,
    pub constant: bool,
    pub dfk: bool,
    pub exogenous_names: Vec<String>,
    /// Zero-based source rows used for every equation.
    pub sample_rows: Vec<usize>,
    /// Equation-major coefficients and residuals.
    pub coefficients: Vec<Vec<f64>>,
    pub residuals: Vec<Vec<f64>>,
    /// Observation-major fitted design, retained for residual diagnostics.
    pub design: Vec<Vec<f64>>,
    pub sigma: Vec<Vec<f64>>,
    pub statistics: MultivariateStatistics,
    pub fpe: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct VarSummaryOptions {
    pub model_summary: bool,
    pub coefficient_table: bool,
    pub lag_exclusion: bool,
    pub serial_tests: bool,
    pub stability: bool,
    pub serial_lags: usize,
}

impl Default for VarSummaryOptions {
    fn default() -> Self {
        Self {
            model_summary: true,
            coefficient_table: true,
            lag_exclusion: false,
            serial_tests: false,
            stability: false,
            serial_lags: 2,
        }
    }
}

/// Explicit selected-lag VAR design; columns stay in input order.
pub struct VarOptions {
    pub lags: Vec<usize>,
    pub constant: bool,
    pub dfk: bool,
    pub variable_names: Vec<String>,
    pub exogenous_names: Vec<String>,
}
