//! Model comparison, observation influence and questionnaire diagnostic contracts.
use crate::regression::{fit::RegressionFit, linear::LinearRegressionResult};
use serde::Serialize;

#[derive(Clone, Copy)]
pub enum DiagnosticModel<'a> {
    Linear(&'a LinearRegressionResult),
    Binary(&'a RegressionFit),
}

#[derive(Debug, Clone, Serialize)]
pub struct InformationCriteria {
    pub family: String,
    pub observations: usize,
    /// Includes the estimated Gaussian error variance; binary scale is fixed.
    pub parameters: usize,
    pub log_likelihood: f64,
    pub aic: f64,
    pub bic: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonTest {
    pub statistic: f64,
    pub degrees_of_freedom: usize,
    pub denominator_df: Option<usize>,
    pub p_value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonMethod {
    LikelihoodRatio,
    Score,
    All,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelComparison {
    pub restricted: InformationCriteria,
    pub full: InformationCriteria,
    pub restrictions: usize,
    pub likelihood_ratio: Option<ComparisonTest>,
    pub score: Option<ComparisonTest>,
    pub f_test: Option<ComparisonTest>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InfluenceSummary {
    pub observations: usize,
    pub parameters: usize,
    pub residual_degrees_of_freedom: usize,
    pub residual_mean: f64,
    pub residual_sd: f64,
    pub residual_minimum: f64,
    pub residual_maximum: f64,
    pub weighted_residual_sum_squares: f64,
    pub residual_standard_error: f64,
    pub maximum_leverage: f64,
    pub maximum_cooks_distance: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct InfluenceOutput {
    pub summary: InfluenceSummary,
    /// All observation vectors follow the fitted sample order.
    pub leverage: Vec<f64>,
    pub standardized_residuals: Vec<Option<f64>>,
    pub studentized_residuals: Vec<Option<f64>>,
    pub cooks_distance: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollinearityTerm {
    pub term: String,
    pub constant: bool,
    /// None when undefined or unbounded; tolerance distinguishes perfect dependence.
    pub vif: Option<f64>,
    pub tolerance: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollinearityResult {
    pub observations: usize,
    pub columns: usize,
    pub rank: usize,
    pub full_column_rank: bool,
    pub centered: bool,
    pub condition_number: Option<f64>,
    pub eigenvalues: Vec<f64>,
    pub condition_indices: Vec<Option<f64>>,
    pub terms: Vec<CollinearityTerm>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HarmanResult {
    pub extraction: String,
    pub observations: usize,
    pub variables: usize,
    pub eigenvalues: Vec<f64>,
    pub explained_variance_ratio: Vec<f64>,
    pub first_component_ratio: f64,
    pub first_component_loadings: Vec<f64>,
    pub eigenvalues_above_one: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReclassificationMode {
    Continuous,
    Categorical,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReclassificationCounts {
    pub observations: usize,
    pub upward: usize,
    pub downward: usize,
    pub unchanged: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReclassificationResult {
    pub mode: ReclassificationMode,
    pub thresholds: Vec<f64>,
    pub events: ReclassificationCounts,
    pub nonevents: ReclassificationCounts,
    pub nri_events: f64,
    pub nri_nonevents: f64,
    pub nri: f64,
    pub discrimination_slope_reference: f64,
    pub discrimination_slope_new: f64,
    pub idi: f64,
}
