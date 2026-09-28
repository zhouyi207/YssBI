//! Residual diagnostic selections and numerical results.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BreuschPaganResult {
    /// LM 统计量
    pub lm_stat: f64,
    /// 自由度
    pub df: usize,
    /// p 值 (H0: 同方差)
    pub p_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Chi2TestResult {
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImTestResult {
    pub heteroskedasticity: Chi2TestResult,
    pub skewness: Chi2TestResult,
    pub kurtosis: Chi2TestResult,
    pub total: Chi2TestResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResetTestResult {
    pub f_stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NormalityTestResult {
    pub skewness: f64,
    pub kurtosis: f64,
    pub omnibus_stat: f64,
    pub omnibus_p_value: f64,
    pub jarque_bera_stat: f64,
    pub jarque_bera_p_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VifEntry {
    pub vif: f64,
    pub tolerance: f64, // 1/VIF
}

#[derive(Debug, Clone, Copy)]
pub enum ResidualDiagnostic {
    BreuschPagan { rhs: bool, koenker: bool },
    White,
    InformationMatrix,
    Reset { rhs: bool },
    Vif,
    Leverage,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "test", content = "result", rename_all = "snake_case")]
pub enum ResidualDiagnosticResult {
    BreuschPagan(BreuschPaganResult),
    White(BreuschPaganResult),
    InformationMatrix(ImTestResult),
    Reset(ResetTestResult),
    Vif(Vec<VifEntry>),
    Leverage(Vec<f64>),
}
