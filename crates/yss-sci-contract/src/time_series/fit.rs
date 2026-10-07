//! Shared computed statistics for multivariate time-series models.
use crate::regression::fit::RegressionCoefficientStatistics;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquationStatistics {
    pub eq_name: String,
    pub parms: usize,
    pub rmse: f64,
    pub r_sq: f64,
    pub chi2: f64,
    pub p_chi2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultivariateStatistics {
    pub observations: usize,
    pub log_likelihood: f64,
    pub aic: f64,
    pub hqic: f64,
    pub sbic: f64,
    pub det_sigma_ml: f64,
    pub equations: Vec<EquationStatistics>,
    pub coefficients: Vec<RegressionCoefficientStatistics>,
    pub coefficient_labels: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityRoot {
    pub re: f64,
    pub im: f64,
    pub modulus: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialCorrelationTest {
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}
