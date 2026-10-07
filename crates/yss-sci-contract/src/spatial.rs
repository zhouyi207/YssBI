//! Coordinate weights, spatial dependence and Gaussian spatial regression.
use crate::regression::models::{IterationOptions, RegressionCoefficient};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightRule {
    Knn,
    DistanceBand,
    InverseDistance,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeightsOptions {
    pub rule: WeightRule,
    pub neighbors: usize,
    pub radius: f64,
    pub power: f64,
    pub symmetrize: bool,
    pub row_standardize: bool,
}
impl Default for WeightsOptions {
    fn default() -> Self {
        Self {
            rule: WeightRule::Knn,
            neighbors: 4,
            radius: 1.0,
            power: 1.0,
            symmetrize: false,
            row_standardize: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialWeights<L = usize> {
    /// Exact identifiers in matrix row/column order.
    pub units: Vec<L>,
    pub matrix: Vec<Vec<f64>>,
    pub options: WeightsOptions,
    /// Zero-neighbor rows, indexed from zero into units.
    pub islands: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct MoranOptions {
    pub permutations: usize,
    pub seed: u64,
}
impl Default for MoranOptions {
    fn default() -> Self {
        Self {
            permutations: 999,
            seed: 42,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MoranResult {
    pub observations: usize,
    pub statistic: f64,
    pub expected: f64,
    pub normal_variance: Option<f64>,
    pub normal_z: Option<f64>,
    pub normal_p_value: Option<f64>,
    pub randomization_variance: Option<f64>,
    pub randomization_z: Option<f64>,
    pub randomization_p_value: Option<f64>,
    pub permutations: usize,
    pub seed: u64,
    /// Two-sided, centered at E[I], with the Monte Carlo +1 correction.
    pub permutation_p_value: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialMethod {
    Ols,
    Slm,
    Sem,
    Sac,
    Sdm,
    Sdem,
    Slx,
}
impl SpatialMethod {
    pub fn lag_y(self) -> bool {
        matches!(self, Self::Slm | Self::Sac | Self::Sdm)
    }
    pub fn lag_error(self) -> bool {
        matches!(self, Self::Sem | Self::Sac | Self::Sdem)
    }
    pub fn lag_x(self) -> bool {
        matches!(self, Self::Sdm | Self::Sdem | Self::Slx)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct SpatialOptions {
    pub constant: bool,
    pub iteration: IterationOptions,
}
impl Default for SpatialOptions {
    fn default() -> Self {
        Self {
            constant: true,
            iteration: IterationOptions::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpatialImpact {
    pub term: String,
    pub direct: f64,
    pub indirect: f64,
    pub total: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpatialRegressionResult {
    pub method: SpatialMethod,
    pub observations: usize,
    pub estimation_observations: usize,
    pub constant: bool,
    pub periods: usize,
    pub coefficients: Vec<RegressionCoefficient>,
    /// Same order as coefficients, excluding the nuisance log variance.
    pub covariance: Vec<Vec<f64>>,
    pub rho: Option<f64>,
    pub lambda: Option<f64>,
    pub spatial_parameter_bound: f64,
    pub sigma_squared: f64,
    pub log_likelihood: f64,
    pub aic: f64,
    pub bic: f64,
    pub df_residual: usize,
    pub iterations: usize,
    /// Conditional fitted values include the observed Wy term.
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    /// Innovations apply the fitted spatial error filter to residuals.
    pub innovations: Vec<f64>,
    pub reduced_fitted: Vec<f64>,
    pub impacts: Vec<SpatialImpact>,
    /// Descriptive Moran I of innovations by period; no regression-residual p value.
    pub innovation_moran_i: Vec<Option<f64>>,
    /// Entity fixed effects in weights order, empty for cross sections.
    pub unit_effects: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpatialModelReport<L> {
    #[serde(flatten)]
    pub model: SpatialRegressionResult,
    pub unit_labels: Vec<L>,
    pub period_labels: Vec<L>,
    /// Codes into the label arrays, preserving the input observation order.
    pub observation_units: Vec<usize>,
    pub observation_periods: Vec<usize>,
}
