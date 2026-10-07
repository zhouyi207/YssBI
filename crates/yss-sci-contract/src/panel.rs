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

pub const MAX_COINTEGRATION_PREDICTORS: usize = 5;

/// Aligned long-form observations. Time is an integer period index for lagged analyses.
pub struct PanelData<'a> {
    pub response: &'a [f64],
    pub predictors: &'a [Vec<f64>],
    pub entity: &'a [f64],
    pub time: &'a [f64],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelDeterministic {
    None,
    Constant,
    Trend,
}

#[derive(Debug, Clone, Copy)]
pub struct PanelTestOptions {
    pub lags: usize,
    pub deterministic: PanelDeterministic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelEntityTest {
    pub entity: f64,
    pub observations: usize,
    pub statistic: f64,
    pub p_value: f64,
    /// Cointegrating equation: intercept, optional trend, then predictors.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub cointegrating_coefficients: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelFisherTest {
    pub method: String,
    pub deterministic: PanelDeterministic,
    pub lags: usize,
    pub observations: usize,
    /// None denotes an infinite Fisher statistic when an individual p-value underflows to zero.
    pub statistic: Option<f64>,
    pub degrees_of_freedom: usize,
    pub p_value: f64,
    pub entity_tests: Vec<PanelEntityTest>,
}

#[derive(Debug, Clone, Copy)]
pub struct DynamicPanelOptions {
    /// Collapsed level instruments y[t-2], ..., y[t-max_instrument_lag].
    pub max_instrument_lag: usize,
    pub robust: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicPanelFit {
    pub method: String,
    pub covariance: String,
    pub observations: usize,
    pub entities: usize,
    pub time_periods: usize,
    pub instruments: usize,
    pub max_instrument_lag: usize,
    pub parameter_names: Vec<String>,
    pub coefficients: Vec<f64>,
    pub inference: RegressionCoefficientStatistics,
    /// Original row indices of the retained differenced equations, in entity/time order.
    pub source_rows: Vec<usize>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
}
