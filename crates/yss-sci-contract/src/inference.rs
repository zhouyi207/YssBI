//! Confidence intervals and multiplicity-controlled comparisons of independent groups.
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct IntervalOptions {
    pub confidence_level: f64,
    pub degrees_of_freedom: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct IntervalSummary {
    pub rows: usize,
    pub confidence_level: f64,
    pub reference_distribution: &'static str,
    pub degrees_of_freedom: Option<f64>,
}
#[derive(Debug, Clone)]
pub struct IntervalRow {
    pub index: usize,
    pub estimate: f64,
    pub standard_error: f64,
    pub lower: f64,
    pub upper: f64,
}
#[derive(Debug, Clone)]
pub struct IntervalResult {
    pub summary: IntervalSummary,
    pub rows: Vec<IntervalRow>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonAdjustment {
    None,
    Bonferroni,
    Holm,
}
#[derive(Debug, Clone, Copy)]
pub struct PairwiseOptions {
    pub equal_variances: bool,
    pub adjustment: ComparisonAdjustment,
    pub confidence_level: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct ComparisonGroup {
    pub group: usize,
    pub observations: usize,
    pub mean: f64,
    pub standard_deviation: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PairwiseSummary {
    pub observations: usize,
    pub groups: Vec<ComparisonGroup>,
    pub method: &'static str,
    pub adjustment: ComparisonAdjustment,
    pub confidence_level: f64,
    pub comparisons: usize,
    pub intervals_adjusted: bool,
}
#[derive(Debug, Clone)]
pub struct PairwiseRow {
    pub group_a: usize,
    pub group_b: usize,
    pub estimate: f64,
    pub standard_error: f64,
    pub degrees_of_freedom: f64,
    pub statistic: f64,
    pub p_value: f64,
    pub adjusted_p_value: f64,
    pub lower: f64,
    pub upper: f64,
}
#[derive(Debug, Clone)]
pub struct PairwiseResult {
    pub summary: PairwiseSummary,
    pub rows: Vec<PairwiseRow>,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClusterInference {
    pub observations: usize,
    pub clusters: usize,
    pub parameters: usize,
    pub degrees_of_freedom: usize,
    pub covariance_correction: f64,
    pub r_squared: f64,
    pub coefficients: Vec<crate::regression::models::RegressionCoefficient>,
    pub covariance: Vec<Vec<f64>>,
}
