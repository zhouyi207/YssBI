//! Shared hypothesis alternatives and computed results.
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
