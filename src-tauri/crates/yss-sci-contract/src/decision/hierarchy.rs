use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct AhpResult {
    pub criteria: usize,
    pub weights: Vec<f64>,
    pub principal_eigenvalue: f64,
    pub consistency_index: f64,
    pub random_index: Option<f64>,
    pub random_index_source: &'static str,
    pub consistency_ratio: Option<f64>,
    pub consistency_satisfied: Option<bool>,
}
#[derive(Debug, Clone, Serialize)]
pub struct FuzzyAhpResult {
    pub criteria: usize,
    pub weights: Vec<f64>,
    pub compatibility_index: f64,
    pub consistency_satisfied: bool,
}
