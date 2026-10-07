//! Single-stage, with-replacement survey designs and Taylor-linearized estimates.
use crate::regression::models::{GlmFamily, IterationOptions, RegressionModelResult};
use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LonelyPsu {
    Fail,
    Certainty,
}
#[derive(Debug, Clone, Copy)]
pub struct SurveyDesign<'a> {
    pub weights: &'a [f64],
    pub strata: Option<&'a [usize]>,
    pub clusters: Option<&'a [usize]>,
    pub lonely_psu: LonelyPsu,
}
#[derive(Debug, Clone, Serialize)]
pub struct WeightSummary {
    pub observations: usize,
    pub sum: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub kish_effective_n: f64,
    pub weight_design_effect: f64,
    pub coefficient_of_variation: f64,
}
pub struct SurveyWeights {
    pub weights: Vec<f64>,
    pub summary: WeightSummary,
}
#[derive(Debug, Clone, Serialize)]
pub struct SurveyDesignSummary {
    pub strata: usize,
    pub primary_sampling_units: usize,
    pub degrees_of_freedom: usize,
    pub certainty_strata: usize,
    pub variance_method: String,
    pub weights: WeightSummary,
}
#[derive(Debug, Clone, Serialize)]
pub struct SurveyMean {
    pub kind: String,
    pub estimate: f64,
    pub standard_error: f64,
    pub confidence_interval: [f64; 2],
    pub boundary_proportion: bool,
    pub design: SurveyDesignSummary,
}
#[derive(Debug, Clone, Copy)]
pub struct SurveyRegressionOptions {
    pub family: GlmFamily,
    pub constant: bool,
    pub iteration: IterationOptions,
}
#[derive(Debug, Clone, Serialize)]
pub struct SurveyRegressionDiagnostics {
    pub design: SurveyDesignSummary,
    pub coefficient_degrees_of_freedom: usize,
    pub family: GlmFamily,
}
pub struct SurveyRegression {
    pub model: RegressionModelResult,
    pub diagnostics: SurveyRegressionDiagnostics,
}
