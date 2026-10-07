//! Right-censored survival estimates, regression models and evaluation plots.
use crate::regression::models::{IterationOptions, RegressionCoefficient};
use crate::visualization::XyPlot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveMethod {
    KaplanMeier,
    NelsonAalen,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SurvivalPoint {
    pub time: f64,
    pub at_risk: usize,
    pub events: usize,
    pub censored: usize,
    pub survival: f64,
    pub cumulative_hazard: f64,
    pub estimate: f64,
    pub standard_error: f64,
    pub confidence_interval: [f64; 2],
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SurvivalCurve<L = usize> {
    pub group: L,
    pub observations: usize,
    pub events: usize,
    pub median_survival: Option<f64>,
    pub points: Vec<SurvivalPoint>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CurveResult<L = usize> {
    pub method: CurveMethod,
    pub confidence_level: f64,
    pub curves: Vec<SurvivalCurve<L>>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SurvivalTest {
    pub statistic: f64,
    pub degrees_of_freedom: usize,
    pub p_value: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogrankResult<L = usize> {
    pub groups: Vec<L>,
    pub observed: Vec<usize>,
    pub expected: Vec<f64>,
    pub covariance: Vec<Vec<f64>>,
    pub test: SurvivalTest,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IncidencePoint {
    pub time: f64,
    pub at_risk: usize,
    pub events: Vec<usize>,
    pub censored: usize,
    pub survival: f64,
    pub cumulative_incidence: Vec<f64>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompetingRisksResult {
    pub observations: usize,
    pub causes: Vec<usize>,
    pub points: Vec<IncidencePoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoxTies {
    Efron,
    Breslow,
}
#[derive(Debug, Clone, Copy)]
pub struct CoxOptions {
    pub ties: CoxTies,
    pub iteration: IterationOptions,
}
impl Default for CoxOptions {
    fn default() -> Self {
        Self {
            ties: CoxTies::Efron,
            iteration: IterationOptions::default(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatioEstimate {
    pub term: String,
    pub estimate: f64,
    pub confidence_interval: Option<[f64; 2]>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselinePoint {
    pub time: f64,
    pub cumulative_hazard: f64,
    pub survival: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoxBaseline {
    pub stratum: usize,
    pub maximum_followup: f64,
    pub points: Vec<BaselinePoint>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoxResult {
    pub observations: usize,
    pub events: usize,
    pub ties: CoxTies,
    pub iterations: usize,
    pub log_likelihood: f64,
    pub coefficients: Vec<RegressionCoefficient>,
    pub covariance: Vec<Vec<f64>>,
    pub hazard_ratios: Vec<RatioEstimate>,
    pub predictor_means: Vec<f64>,
    pub predictor_ranges: Vec<[f64; 2]>,
    /// Linear predictors centered at predictor_means; baseline uses the same origin.
    pub linear_predictors: Vec<f64>,
    /// Breslow cumulative baseline hazards, including the event at each listed time.
    pub baselines: Vec<CoxBaseline>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhTimeTransform {
    Rank,
    Log,
    Identity,
}
#[derive(Debug, Clone, Serialize)]
pub struct PhTermTest {
    pub term: String,
    pub test: SurvivalTest,
}
#[derive(Debug, Clone, Serialize)]
pub struct ProportionalHazardsResult {
    pub observations: usize,
    pub events: usize,
    pub ties: CoxTies,
    pub time_transform: PhTimeTransform,
    pub log_likelihood: f64,
    pub coefficients: Vec<RegressionCoefficient>,
    pub terms: Vec<PhTermTest>,
    pub global: SurvivalTest,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AftDistribution {
    Exponential,
    Weibull,
    Lognormal,
    Loglogistic,
}
#[derive(Debug, Clone, Copy)]
pub struct AftOptions {
    pub distribution: AftDistribution,
    pub iteration: IterationOptions,
    pub horizon: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AftResult {
    pub distribution: AftDistribution,
    pub observations: usize,
    pub events: usize,
    pub iterations: usize,
    pub coefficients: Vec<RegressionCoefficient>,
    pub time_ratios: Vec<RatioEstimate>,
    /// Raw coefficients followed by log(scale) for estimated-scale distributions.
    pub covariance: Vec<Vec<f64>>,
    pub scale: f64,
    pub scale_standard_error: Option<f64>,
    pub log_likelihood: f64,
    pub aic: f64,
    pub bic: f64,
    pub median_survival: Vec<f64>,
    pub horizon: f64,
    pub event_probabilities: Vec<f64>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubgroupResult<L = usize> {
    pub groups: Vec<L>,
    pub group_observations: Vec<usize>,
    pub group_events: Vec<usize>,
    pub treatment_hazard_ratios: Vec<RatioEstimate>,
    pub equality_test: SurvivalTest,
    pub model: CoxResult,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CalibrationBin {
    pub observations: usize,
    pub predicted_range: [f64; 2],
    pub mean_prediction: f64,
    pub observed_risk: f64,
    pub confidence_interval: [f64; 2],
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CalibrationPlot {
    #[serde(flatten)]
    pub plot: XyPlot,
    pub horizon: f64,
    pub bins: Vec<CalibrationBin>,
}
#[derive(Debug, Clone, Copy)]
pub struct DecisionOptions {
    pub horizon: f64,
    pub minimum_threshold: f64,
    pub maximum_threshold: f64,
    pub points: usize,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DecisionPoint {
    pub threshold: f64,
    pub selected: usize,
    pub model: f64,
    pub treat_all: f64,
    pub treat_none: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DecisionPlot {
    #[serde(flatten)]
    pub plot: XyPlot,
    pub horizon: f64,
    pub estimates: Vec<DecisionPoint>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NomogramTick {
    pub position: f64,
    pub label: String,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NomogramAxis {
    pub label: String,
    pub ticks: Vec<NomogramTick>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NomogramPlot {
    pub axes: Vec<NomogramAxis>,
    pub horizon: f64,
    pub maximum_total_points: f64,
    pub points_per_log_hazard: f64,
    pub baseline_cumulative_hazard: f64,
}
