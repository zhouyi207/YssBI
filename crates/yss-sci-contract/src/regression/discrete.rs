//! Configuration of binary-response estimation and prediction.
use super::postestimation::Evaluation;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinaryOptions {
    pub constant: bool,
    pub max_iterations: usize,
    pub tolerance: f64,
}

impl Default for BinaryOptions {
    fn default() -> Self {
        Self {
            constant: true,
            max_iterations: 100,
            tolerance: 1e-8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarginalMethod {
    Dydx,
    Eyex,
    Eydx,
    Dyex,
}
#[derive(Debug, Clone)]
pub struct MarginalOptions {
    pub evaluation: Evaluation,
    pub method: MarginalMethod,
    pub at: std::collections::HashMap<String, f64>,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct EffectInference {
    pub variable: String,
    pub estimate: f64,
    pub standard_error: f64,
    pub z_value: Option<f64>,
    pub p_value: Option<f64>,
    pub ci_lower: f64,
    pub ci_upper: f64,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct MarginalEffects {
    pub evaluation: Evaluation,
    pub method: MarginalMethod,
    pub at: std::collections::HashMap<String, f64>,
    pub coefficients: Vec<EffectInference>,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct BinaryClassification {
    pub cutoff: f64,
    pub true_positive: usize,
    pub false_positive: usize,
    pub false_negative: usize,
    pub true_negative: usize,
    pub sensitivity: Option<f64>,
    pub specificity: Option<f64>,
    pub positive_predictive_value: Option<f64>,
    pub negative_predictive_value: Option<f64>,
    pub accuracy: f64,
    pub error_rate: f64,
}
