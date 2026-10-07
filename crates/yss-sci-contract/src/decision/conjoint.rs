use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct PartWorth {
    pub level: usize,
    pub utility: f64,
    pub standard_error: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ConjointAttribute {
    pub attribute: usize,
    pub range: f64,
    pub importance_percent: Option<f64>,
    pub levels: Vec<PartWorth>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ConjointSummary {
    pub method: &'static str,
    pub observations: usize,
    pub parameters: usize,
    pub residual_degrees_of_freedom: usize,
    pub centered_intercept: f64,
    pub r_squared: Option<f64>,
    pub attributes: Vec<ConjointAttribute>,
}
#[derive(Debug, Clone)]
pub struct ConjointPrediction {
    pub observation: usize,
    pub observed: f64,
    pub fitted: f64,
    pub residual: f64,
}
#[derive(Debug, Clone)]
pub struct ConjointResult {
    pub summary: ConjointSummary,
    pub rows: Vec<ConjointPrediction>,
}
