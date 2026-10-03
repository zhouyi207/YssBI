//! Observed-variable path and interaction models, independent of graph identities.
use crate::regression::models::RegressionModelResult;
use serde::Serialize;
#[derive(Debug, Clone, Copy)]
pub struct ModerationOptions {
    pub second_moderator: bool,
    pub probe_sd: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct PathEffect {
    pub estimate: f64,
    pub standard_error: Option<f64>,
    pub statistic: Option<f64>,
    pub p_value: Option<f64>,
    pub confidence_interval: Option<[f64; 2]>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ConditionalEffect {
    pub moderator: f64,
    pub second_moderator: Option<f64>,
    pub in_observed_ranges: bool,
    #[serde(flatten)]
    pub effect: PathEffect,
}
#[derive(Debug, Clone, Serialize)]
pub struct JohnsonNeyman {
    pub second_moderator: Option<f64>,
    pub observed_range: [f64; 2],
    /// Only roots and significant intervals within the observed moderator range.
    pub boundaries: Vec<f64>,
    pub significant_ranges: Vec<[f64; 2]>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ModerationDiagnostics {
    /// X, W, optional Z, then covariates, in input order.
    pub input_centers: Vec<f64>,
    pub probe_sd: f64,
    pub effects: Vec<ConditionalEffect>,
    pub johnson_neyman: Vec<JohnsonNeyman>,
}
pub struct ModerationResult {
    pub model: RegressionModelResult,
    pub diagnostics: ModerationDiagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediatedStage {
    None,
    First,
    Second,
}
#[derive(Debug, Clone, Copy)]
pub struct MediationOptions {
    pub moderated_stage: MediatedStage,
    pub probe_sd: f64,
    /// Zero disables bootstrap inference; otherwise at least two replications.
    pub replications: usize,
    pub seed: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct IndirectEffect {
    pub moderator: Option<f64>,
    pub in_observed_range: bool,
    pub indirect: PathEffect,
    pub total: PathEffect,
}
#[derive(Debug, Clone, Serialize)]
pub struct MediationDiagnostics {
    pub moderated_stage: MediatedStage,
    /// X, M, optional W, then covariates, in input order.
    pub input_centers: Vec<f64>,
    pub replications: usize,
    pub seed: u64,
    pub inference: String,
    pub direct: PathEffect,
    pub effects: Vec<IndirectEffect>,
    pub moderated_mediation_index: Option<PathEffect>,
}
pub struct MediationResult {
    pub mediator: RegressionModelResult,
    pub outcome: RegressionModelResult,
    pub diagnostics: MediationDiagnostics,
}

/// Zero-based column indices, independent of names or graph identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEquation {
    pub response: usize,
    pub predictors: Vec<usize>,
}
pub struct PathEquationFit {
    pub equation: PathEquation,
    pub model: RegressionModelResult,
}
#[derive(Debug, Clone, Serialize)]
pub struct PathDecomposition {
    pub source: usize,
    pub target: usize,
    pub direct: f64,
    pub indirect: f64,
    pub total: f64,
    pub standardized_total: Option<f64>,
}
pub struct RecursivePathResult {
    pub observations: usize,
    pub equations: Vec<PathEquationFit>,
    pub effects: Vec<PathDecomposition>,
}
