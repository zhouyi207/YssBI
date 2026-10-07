//! VEC fit facts, separated from residual and stability diagnostics.
use super::fit::MultivariateStatistics;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VecFit {
    pub var_names: Vec<String>,
    pub rank: usize,
    pub lags: usize,
    pub trend_spec: String,
    pub coefficients: Vec<Vec<f64>>,
    pub residuals: Vec<Vec<f64>>,
    pub design: Vec<Vec<f64>>,
    pub statistics: MultivariateStatistics,
    pub cointegration: CointegrationStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CointegrationStatistics {
    pub beta: Vec<Vec<f64>>,
    pub equations: Vec<CointegratingEquationStatistics>,
    /// Normalized coefficients and deterministic terms have no estimated interval.
    pub standard_errors: Vec<Vec<Option<f64>>>,
    pub statistic_values: Vec<Vec<Option<f64>>>,
    pub p_values: Vec<Vec<Option<f64>>>,
    pub confidence_interval_lower: Vec<Vec<Option<f64>>>,
    pub confidence_interval_upper: Vec<Vec<Option<f64>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CointegratingEquationStatistics {
    pub eq_name: String,
    pub parms: usize,
    pub chi2: f64,
    pub p_chi2: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct VecSummaryOptions {
    pub model_summary: bool,
    pub coefficient_table: bool,
    pub cointegration: bool,
    pub serial_tests: bool,
    pub stability: bool,
    pub serial_lags: usize,
}

impl Default for VecSummaryOptions {
    fn default() -> Self {
        Self {
            model_summary: true,
            coefficient_table: true,
            cointegration: true,
            serial_tests: false,
            stability: false,
            serial_lags: 2,
        }
    }
}
