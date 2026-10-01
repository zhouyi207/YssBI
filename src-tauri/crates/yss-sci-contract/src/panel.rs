//! Numerical panel-model fits.
use crate::regression::fit::{LinearRegressionStatistics, RegressionCoefficientStatistics};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelFit {
    pub family: String,
    pub parameter_names: Vec<String>,
    pub parameter_categories: Vec<Option<String>>,
    pub response_name: String,
    pub omitted_terms: Vec<PanelOmittedTerm>,
    pub estimation: PanelEstimationSample,
    pub coefficients: Vec<f64>,
    pub inference: RegressionCoefficientStatistics,
    pub statistics: PanelModelStatistics,
    pub recovered_constant: Option<f64>,
    pub recovered_constant_standard_error: Option<f64>,
    pub effects_statistics: Option<PanelFEStats>,
    pub omitted_indices: Option<Vec<usize>>,
    pub estimator_statistics: PanelEstimatorStatistics,
    pub covariance_nonrobust: Option<Vec<Vec<f64>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelModelStatistics {
    pub observations: usize,
    pub entities: usize,
    pub time_periods: usize,
    pub within_r2: Option<f64>,
    pub linear: LinearRegressionStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PanelEstimatorStatistics {
    LeastSquares,
    RandomEffects {
        wald_chi2: f64,
        wald_p_value: f64,
    },
    MaximumLikelihood {
        log_likelihood: f64,
        lr_chi2: f64,
        lr_p_value: f64,
        chibar2: f64,
        chibar2_p_value: f64,
        constant_iterations: Vec<f64>,
        iterations: Vec<f64>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct PanelSummaryOptions {
    pub model_summary: bool,
    pub coefficient_table: bool,
    pub effects_statistics: bool,
    pub estimator_statistics: bool,
}

impl Default for PanelSummaryOptions {
    fn default() -> Self {
        Self {
            model_summary: true,
            coefficient_table: true,
            effects_statistics: true,
            estimator_statistics: true,
        }
    }
}

/// R² Within/Between/Overall. None for MLE (does not report these).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelR2Stats {
    pub r2_within: f64,
    pub r2_between: f64,
    pub r2_overall: f64,
}

/// Observations per group (min/avg/max across entities or time periods).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsPerGroupStats {
    pub min: usize,
    pub avg: f64,
    pub max: usize,
}

/// Variance decomposition: σ_u, σ_e, ρ = σ²_u / (σ²_u + σ²_e).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigmaStats {
    pub sigma_u: f64,
    pub sigma_e: f64,
    pub rho: f64,
}

/// RE quasi-demeaning parameter θ = 1 - sqrt(σ²_e / (T_i·σ²_u + σ²_e)).
/// For balanced panels all three are equal; for unbalanced they differ.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThetaStats {
    pub min: f64,
    pub avg: f64,
    pub max: f64,
}

/// FE-specific stats (Stata xtreg, fe style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelFEStats {
    pub r2: Option<PanelR2Stats>,
    pub obs_per_group: ObsPerGroupStats,
    pub sigma: SigmaStats,
    pub corr_u_i_xb: f64,
    pub theta: Option<ThetaStats>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelEstimator {
    FixedEffects,
    Lsdv,
    FirstDifference,
    RandomEffects,
    MaximumLikelihood,
    Between,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelEffects {
    Entity,
    Time,
    TwoWay,
}
pub struct PanelOptions {
    pub estimator: PanelEstimator,
    pub effects: PanelEffects,
    pub constant: bool,
    pub covariance: String,
}
impl Default for PanelOptions {
    fn default() -> Self {
        Self {
            estimator: PanelEstimator::FixedEffects,
            effects: PanelEffects::TwoWay,
            constant: true,
            covariance: "cluster".into(),
        }
    }
}

/// Every column is on the estimator's actual scale (within, quasi-demeaned,
/// first-difference, between-group means, or original LSDV observations).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelEstimationSample {
    pub space: String,
    pub constant: bool,
    pub response: Vec<f64>,
    pub design: Vec<Vec<f64>>,
    pub coefficients: Vec<f64>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    /// One source row per ordinary observation; two for differences; groups for means.
    pub source_rows: Vec<Vec<usize>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelOmittedTerm {
    pub index: usize,
    pub variable: String,
    pub category: Option<String>,
    pub reason: String,
}
