//! Independent study effects, inverse-variance models and meta-analysis diagnostics.
use crate::regression::models::RegressionCoefficient;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetaEstimator {
    Fixed,
    DerSimonianLaird,
    PauleMandel,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetaInference {
    Wald,
    KnappHartung,
}
#[derive(Debug, Clone, Copy)]
pub struct MetaOptions {
    pub estimator: MetaEstimator,
    pub inference: MetaInference,
    pub confidence_level: f64,
}
impl Default for MetaOptions {
    fn default() -> Self {
        Self {
            estimator: MetaEstimator::PauleMandel,
            inference: MetaInference::Wald,
            confidence_level: 0.95,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectMeasure {
    MeanDifference,
    HedgesG,
    LogOddsRatio,
    LogRiskRatio,
    RiskDifference,
    Proportion,
    LogitProportion,
    ArcsineProportion,
    Mean,
    FisherZ,
    LogRatio,
}
#[derive(Debug, Clone, Copy)]
pub struct ArmSummary<'a> {
    pub mean: &'a [f64],
    pub sd: &'a [f64],
    pub size: &'a [f64],
}
#[derive(Debug, Clone, Copy)]
pub struct BinomialSummary<'a> {
    pub events: &'a [f64],
    pub total: &'a [f64],
}
#[derive(Debug, Clone, Serialize)]
pub struct EffectSummary {
    pub measure: EffectMeasure,
    pub studies: usize,
    pub confidence_level: f64,
}
#[derive(Debug, Clone)]
pub struct StudyEffect {
    /// Original one-based study position.
    pub study: usize,
    pub effect: f64,
    pub variance: f64,
    pub standard_error: f64,
    pub lower: f64,
    pub upper: f64,
}
#[derive(Debug, Clone)]
pub struct EffectResult {
    pub summary: EffectSummary,
    pub rows: Vec<StudyEffect>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Heterogeneity {
    pub q: f64,
    pub degrees_of_freedom: usize,
    pub p_value: f64,
    pub i_squared_percent: f64,
    pub h_squared: f64,
    pub tau_squared: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct MetaSummary {
    pub studies: usize,
    pub estimator: MetaEstimator,
    pub inference: MetaInference,
    pub confidence_level: f64,
    pub residual_degrees_of_freedom: usize,
    pub coefficients: Vec<RegressionCoefficient>,
    pub covariance: Vec<Vec<f64>>,
    pub heterogeneity: Heterogeneity,
    pub residual_q: f64,
    /// Intercept-only random-effects prediction interval, t(k-2) approximation.
    pub prediction_interval: Option<[f64; 2]>,
}
#[derive(Debug, Clone)]
pub struct MetaStudy {
    pub effect: StudyEffect,
    pub weight: f64,
    pub fitted: f64,
    pub residual: f64,
}
#[derive(Debug, Clone)]
pub struct MetaFit {
    pub summary: MetaSummary,
    pub studies: Vec<MetaStudy>,
}
#[derive(Debug, Clone)]
pub struct OmissionResult {
    pub omitted_study: usize,
    pub estimate: f64,
    pub standard_error: f64,
    pub lower: f64,
    pub upper: f64,
    pub tau_squared: f64,
    pub i_squared_percent: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct SensitivitySummary {
    pub baseline: MetaSummary,
    pub alternative_models: Vec<MetaSummary>,
}
#[derive(Debug, Clone, Serialize)]
pub struct CombinedP {
    pub method: String,
    pub studies: usize,
    /// Infinite limiting statistics have a defined p-value but no finite statistic.
    pub statistic: Option<f64>,
    pub degrees_of_freedom: Option<usize>,
    pub p_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunnelPlot {
    #[serde(flatten)]
    pub plot: crate::visualization::XyPlot,
    /// Descending domain places zero standard error at the top of the funnel.
    #[serde(rename = "yDomain")]
    pub y_domain: [f64; 2],
}
