use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FuzzyOperator {
    ProductSum,
    MinMax,
    ProductMax,
    MinSum,
}
#[derive(Debug, Clone, Serialize)]
pub struct FuzzyEvaluation {
    pub criteria: usize,
    pub grades: usize,
    pub operator: FuzzyOperator,
    pub weights: Vec<f64>,
    pub raw_memberships: Vec<f64>,
    pub memberships: Vec<f64>,
    pub dominant_grades: Vec<usize>,
    pub score: Option<f64>,
}
