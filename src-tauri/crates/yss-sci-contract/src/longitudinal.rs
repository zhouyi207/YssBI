//! Neutral longitudinal estimating equations and mixed-model contracts.
use crate::regression::models::{IterationOptions, RegressionCoefficient};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFamily {
    Gaussian,
    Binomial,
    Poisson,
    NegativeBinomial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkingCorrelation {
    Independence,
    Exchangeable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixedEstimation {
    Ml,
    Reml,
}

#[derive(Debug, Clone)]
pub struct Grouping {
    /// Dense zero-based identifiers; every declared level must be observed.
    pub codes: Vec<usize>,
    pub levels: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct GeeOptions {
    pub family: ResponseFamily,
    pub correlation: WorkingCorrelation,
    pub constant: bool,
    pub iteration: IterationOptions,
}
impl Default for GeeOptions {
    fn default() -> Self {
        Self {
            family: ResponseFamily::Gaussian,
            correlation: WorkingCorrelation::Exchangeable,
            constant: true,
            iteration: IterationOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MixedOptions {
    pub estimation: MixedEstimation,
    pub constant: bool,
    pub nested: bool,
    pub iteration: IterationOptions,
}
impl Default for MixedOptions {
    fn default() -> Self {
        Self {
            estimation: MixedEstimation::Reml,
            constant: true,
            nested: false,
            iteration: IterationOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarianceComponent {
    /// One-based grouping and random-term positions; term 0 is the intercept.
    pub grouping: usize,
    pub term: usize,
    pub variance: f64,
    pub boundary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomEffect {
    pub grouping: usize,
    pub level: usize,
    pub term: usize,
    pub estimate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LongitudinalResult {
    pub method: String,
    pub family: ResponseFamily,
    pub observations: usize,
    pub group_counts: Vec<usize>,
    pub coefficients: Vec<RegressionCoefficient>,
    pub coefficient_covariance: Vec<Vec<f64>>,
    pub inference: String,
    pub iterations: usize,
    pub working_correlation: Option<WorkingCorrelation>,
    pub correlation: Option<f64>,
    pub scale: f64,
    /// ML or restricted likelihood as identified by `method`; GEE has none.
    pub log_likelihood: Option<f64>,
    /// Unavailable for REML or GEE, to avoid invalid fixed-design comparisons.
    pub aic: Option<f64>,
    pub negative_binomial_alpha: Option<f64>,
    pub variance_components: Vec<VarianceComponent>,
    pub random_effects: Vec<RandomEffect>,
    /// Mixed models: inverse-link(X beta), with random effects set to zero.
    /// GEE: population-mean predictions. Neither vector changes input row order.
    pub fixed_fitted: Vec<f64>,
    pub fitted_values: Vec<f64>,
    pub residuals: Vec<f64>,
}
