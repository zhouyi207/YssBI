//! Numeric model fits independent of a matrix backend.
use crate::StatisticalObservationMetadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressionKind {
    Ols,
    Gls,
    Logit,
    Probit,
    Prais,
    Wls,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionFit {
    pub constant: bool,
    pub family: &'static str,
    pub coefficients: Vec<f64>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub statistics: RegressionStatistics,
    pub metadata: StatisticalObservationMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentalVariableKind {
    TwoStageLeastSquares,
    LimitedInformationMaximumLikelihood,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionCoefficientStatistics {
    pub covariance: Vec<Vec<f64>>,
    pub standard_errors: Vec<f64>,
    pub statistic_values: Vec<f64>,
    pub p_values: Vec<f64>,
    pub confidence_interval_lower: Vec<f64>,
    pub confidence_interval_upper: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinearRegressionStatistics {
    pub r2: f64,
    pub adjusted_r2: f64,
    pub f_statistic: f64,
    pub f_p_value: f64,
    pub df_model: usize,
    pub df_residual: usize,
    pub df_total: usize,
    pub ss_model: f64,
    pub ss_residual: f64,
    pub ss_total: f64,
    pub ms_model: f64,
    pub ms_residual: f64,
    pub ms_total: f64,
    pub covariance_type: String,
    pub condition_number: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BinaryRegressionLink {
    Logit,
    Probit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryRegressionStatistics {
    pub log_likelihood: f64,
    pub null_log_likelihood: f64,
    pub pseudo_r2: f64,
    pub adjusted_pseudo_r2: f64,
    pub lr_chi2: f64,
    pub lr_p_value: f64,
    pub aic: f64,
    pub bic: f64,
    pub iterations: usize,
    pub converged: bool,
    pub condition_number: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PraisRegressionStatistics {
    #[serde(flatten)]
    pub linear: LinearRegressionStatistics,
    pub rho: f64,
    pub durbin_watson_original: f64,
    pub durbin_watson_transformed: f64,
    pub iterations: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RegressionStatistics {
    Linear {
        #[serde(flatten)]
        coefficients: RegressionCoefficientStatistics,
        #[serde(flatten)]
        model: LinearRegressionStatistics,
    },
    Binary {
        link: BinaryRegressionLink,
        #[serde(flatten)]
        coefficients: RegressionCoefficientStatistics,
        #[serde(flatten)]
        model: BinaryRegressionStatistics,
    },
    Prais {
        #[serde(flatten)]
        coefficients: RegressionCoefficientStatistics,
        #[serde(flatten)]
        model: PraisRegressionStatistics,
    },
}

impl RegressionStatistics {
    pub fn coefficient_statistics(&self) -> &RegressionCoefficientStatistics {
        match self {
            Self::Linear { coefficients, .. }
            | Self::Binary { coefficients, .. }
            | Self::Prais { coefficients, .. } => coefficients,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentalVariableFit {
    pub family: &'static str,
    pub coefficients: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub p_values: Vec<f64>,
    pub r2: f64,
    pub adjusted_r2: f64,
    pub first_stage_min_eigenvalue: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kappa: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelFit {
    pub family: &'static str,
    pub coefficients: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub p_values: Vec<f64>,
    pub r2: f64,
    pub adjusted_r2: f64,
    pub observations: usize,
    pub entities: usize,
    pub time_periods: usize,
}
