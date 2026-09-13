//! Neutral hypothesis requests, alternatives, computed results and errors.
use crate::SciError;
use serde::Serialize;
/// 备择假设类型（t 检验、Wald 检验共用）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alternative {
    TwoSided,
    Greater,
    Less,
}

#[derive(Debug, Clone, Serialize)]
pub struct TTestResult {
    pub constraint_desc: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df: usize,
    pub p_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaldTestResult {
    pub constraint_desc: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

pub struct HypothesisTestInput {
    pub betas: Vec<f64>,
    pub cov_beta: Vec<Vec<f64>>,
    pub df_residual: usize,
    pub param_names: Vec<String>,
    pub hypothesis: String,
}

pub struct HypothesisTestOutput {
    pub test_type: String,
    pub h0_form: String,
    pub h1_form: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum HypothesisError {
    #[error("hypothesis input is invalid: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Scientific(#[from] SciError),
}

impl From<String> for HypothesisError {
    fn from(detail: String) -> Self {
        Self::InvalidInput(detail)
    }
}
