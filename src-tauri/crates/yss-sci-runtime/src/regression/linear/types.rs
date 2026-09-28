use crate::regression::types::VariableSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept_indices: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraisConfigure {
    pub constant: bool,
    pub transform: String,
}

impl Default for PraisConfigure {
    fn default() -> Self {
        Self {
            constant: true,
            transform: "prais".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraisModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
    pub rho: f64,
}

/// Prais-Winsten / Cochrane-Orcutt 特有诊断信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraisInfo {
    pub rho: f64,
    pub dw_original: f64,
    pub dw_transformed: f64,
    pub iterations: usize,
    /// Iteration log: "Prais iteration N: rho = X.XXXX" for each step
    pub iteration_log: Vec<String>,
}
