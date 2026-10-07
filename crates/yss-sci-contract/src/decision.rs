//! Multi-criteria weights and alternative rankings; inputs are column-major.
use serde::Serialize;
pub mod conjoint;
pub mod experts;
pub mod fuzzy;
pub mod hierarchy;
pub mod influence;
pub mod market;
pub mod preferences;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightMethod {
    Equal,
    Explicit,
    Entropy,
    Critic,
    Information,
    Independence,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Normalization {
    None,
    MinMax,
    Vector,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RankingMethod {
    Composite,
    Topsis,
    GreyRelational,
    Wrsr,
    Efficacy,
}
#[derive(Debug, Clone)]
pub struct RankingOptions {
    pub method: RankingMethod,
    pub weighting: WeightMethod,
    pub normalization: Normalization,
    /// True indicates that a smaller observed value is preferred.
    pub costs: Vec<bool>,
    pub weights: Vec<f64>,
    pub grey_resolution: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct WeightResult {
    pub method: WeightMethod,
    pub weights: Vec<f64>,
    /// Entropy, CRITIC information, CV, or multiple correlation, in column order.
    pub statistic: Vec<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct RankingSummary {
    pub observations: usize,
    pub criteria: usize,
    pub method: RankingMethod,
    pub normalization: Normalization,
    pub cost_criteria: Vec<usize>,
    pub weighting: WeightResult,
}
#[derive(Debug, Clone)]
pub struct ScoreRow {
    pub observation: usize,
    pub score: f64,
    pub rank: f64,
    pub distance_best: f64,
    pub distance_worst: f64,
}
#[derive(Debug, Clone)]
pub struct RankingResult {
    pub summary: RankingSummary,
    pub rows: Vec<ScoreRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VikorSummary {
    pub observations: usize,
    pub criteria: usize,
    pub weights: Vec<f64>,
    pub majority_weight: f64,
    pub acceptable_advantage: bool,
    pub acceptable_stability: bool,
    pub compromise_alternatives: Vec<usize>,
}
#[derive(Debug, Clone)]
pub struct VikorRow {
    pub observation: usize,
    pub compromise_score: f64,
    pub group_utility: f64,
    pub individual_regret: f64,
    pub rank: f64,
}
#[derive(Debug, Clone)]
pub struct VikorResult {
    pub summary: VikorSummary,
    pub rows: Vec<VikorRow>,
}
#[derive(Debug, Clone, Serialize)]
pub struct SystemSummary {
    pub method: &'static str,
    pub observations: usize,
    pub criteria: usize,
    pub weights: Vec<f64>,
    pub undefined_rows: usize,
}
#[derive(Debug, Clone)]
pub struct CouplingRow {
    pub observation: usize,
    pub coupling: Option<f64>,
    pub coordination_index: f64,
    pub coordination_degree: f64,
}
#[derive(Debug, Clone)]
pub struct CouplingResult {
    pub summary: SystemSummary,
    pub rows: Vec<CouplingRow>,
}
#[derive(Debug, Clone)]
pub struct ObstacleRow {
    pub observation: usize,
    pub criterion: usize,
    pub deviation: f64,
    pub weighted_deviation: f64,
    pub obstacle_percent: Option<f64>,
}
#[derive(Debug, Clone)]
pub struct ObstacleResult {
    pub summary: SystemSummary,
    pub rows: Vec<ObstacleRow>,
}
