//! Evaluation settings and adjusted means of retained regression designs.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evaluation {
    Average,
    AtMeans,
}

#[derive(Debug, Clone)]
pub struct PredictionOptions {
    pub evaluation: Evaluation,
    pub at: HashMap<String, f64>,
    pub confidence_level: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdjustedPrediction {
    pub observations: usize,
    pub family: &'static str,
    pub evaluation: Evaluation,
    pub at: HashMap<String, f64>,
    pub confidence_level: f64,
    pub degrees_of_freedom: Option<f64>,
    pub estimate: f64,
    pub standard_error: f64,
    pub lower: f64,
    pub upper: f64,
}
