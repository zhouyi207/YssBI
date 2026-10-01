//! Neutral contracts for econometric estimators and observational treatment effects.
use crate::regression::models::{IterationOptions, RegressionCoefficient};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreatmentMethod {
    Matching,
    Ipw,
    RegressionAdjustment,
    Aipw,
}

#[derive(Debug, Clone, Copy)]
pub struct BootstrapOptions {
    pub replications: usize,
    pub seed: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct TreatmentOptions {
    pub method: TreatmentMethod,
    pub overlap: f64,
    pub caliper: f64,
    pub iteration: IterationOptions,
    pub bootstrap: BootstrapOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreatmentEffect {
    pub estimate: f64,
    pub standard_error: Option<f64>,
    pub statistic: Option<f64>,
    pub p_value: Option<f64>,
    pub confidence_interval: Option<[f64; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreatmentResult {
    pub method: TreatmentMethod,
    pub observations: usize,
    pub treated: usize,
    pub controls: usize,
    pub ate: TreatmentEffect,
    pub att: TreatmentEffect,
    pub inference: String,
    pub bootstrap_replications: usize,
    pub propensity_scores: Option<Vec<f64>>,
    pub potential_outcomes: Option<[Vec<f64>; 2]>,
    /// Opposite-treatment nearest-neighbour means, in original row order.
    pub matched_outcomes: Option<Vec<f64>>,
    pub match_counts: Option<Vec<usize>>,
    pub maximum_match_distance: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct GmmOptions {
    pub constant: bool,
    pub two_step: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CausalWaldTest {
    pub statistic: f64,
    pub degrees_of_freedom: usize,
    pub p_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GmmResult {
    pub observations: usize,
    pub instruments: usize,
    pub steps: usize,
    pub coefficients: Vec<RegressionCoefficient>,
    pub covariance: Vec<Vec<f64>>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub moments: Vec<f64>,
    pub hansen_j: Option<CausalWaldTest>,
}

#[derive(Debug, Clone, Copy)]
pub struct RddOptions {
    pub cutoff: f64,
    pub bandwidth: f64,
    pub triangular: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RddResult {
    pub cutoff: f64,
    pub bandwidth: f64,
    pub kernel: String,
    pub left_observations: usize,
    pub right_observations: usize,
    pub coefficients: Vec<RegressionCoefficient>,
    pub covariance: Vec<Vec<f64>>,
    /// Positions start at one in the aligned input.
    pub rows: Vec<usize>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HeterogeneityResult<L = usize> {
    pub observations: usize,
    pub groups: Vec<L>,
    pub group_effects: Vec<RegressionCoefficient>,
    pub equality_test: CausalWaldTest,
    pub coefficients: Vec<RegressionCoefficient>,
    pub covariance: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Copy)]
pub struct HeckmanOptions {
    pub iteration: IterationOptions,
    pub bootstrap: BootstrapOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HeckmanResult {
    pub observations: usize,
    pub selected_observations: usize,
    pub selection_coefficients: Vec<RegressionCoefficient>,
    pub outcome_coefficients: Vec<RegressionCoefficient>,
    pub outcome_covariance: Option<Vec<Vec<f64>>>,
    pub selection_probabilities: Vec<f64>,
    pub inverse_mills: Vec<f64>,
    pub selected_rows: Vec<usize>,
    pub fitted_selected: Vec<f64>,
    pub residuals_selected: Vec<f64>,
    pub sigma: f64,
    pub rho: f64,
    pub bootstrap_replications: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct FrontierOptions {
    pub constant: bool,
    pub cost: bool,
    pub iteration: IterationOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FrontierResult {
    pub observations: usize,
    pub cost: bool,
    pub coefficients: Vec<RegressionCoefficient>,
    pub covariance: Vec<Vec<f64>>,
    pub sigma_u: f64,
    pub sigma_v: f64,
    pub log_likelihood: f64,
    pub iterations: usize,
    pub frontier: Vec<f64>,
    pub residuals: Vec<f64>,
    pub conditional_inefficiency: Vec<f64>,
    /// E[exp(-u) | residual], also for cost models (cost-efficiency ratio).
    pub efficiency: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SurEquation {
    /// One-based indices in the shared predictor input group.
    pub predictors: Vec<usize>,
    pub coefficients: Vec<RegressionCoefficient>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SurResult {
    pub observations: usize,
    pub equations: Vec<SurEquation>,
    pub error_covariance: Vec<Vec<f64>>,
    /// Equation-major order; includes cross-equation coefficient covariance.
    pub coefficient_covariance: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Copy)]
pub struct SyntheticControlOptions {
    pub pre_periods: usize,
    pub iteration: IterationOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SyntheticControlResult {
    pub pre_periods: usize,
    pub post_periods: usize,
    pub donor_weights: Vec<f64>,
    pub synthetic: Vec<f64>,
    pub gaps: Vec<f64>,
    pub post_effect: f64,
    pub pre_rmspe: f64,
    pub post_rmspe: f64,
    pub iterations: usize,
}
