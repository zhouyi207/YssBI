use crate::regression::types::Coefficient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iv2slsOveridDims {
    pub k_iv: usize,
    pub k_endog: usize,
}

/// IV:2SLS 传统豪斯曼检验（Stata hausman iv ols, constant sigmamore）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iv2slsHausmanTest {
    pub stat: f64,
    pub p_value: f64,
    pub df: usize,
}

/// IV:2SLS Durbin-Wu-Hausman 内生性检验（Stata estat endogenous）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iv2slsEndogenousTest {
    pub durbin_stat: f64,
    pub durbin_p_value: f64,
    pub wu_stat: f64,
    pub wu_p_value: f64,
    pub df: usize,
    pub wu_df_denom: usize,
}

/// IV:LIML 过度识别检验（Stata estat overid）
/// Anderson-Rubin (1950) chi2, Basmann F
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IvLimlOveridTest {
    pub anderson_rubin_stat: f64,
    pub anderson_rubin_p_value: f64,
    pub basmann_stat: f64,
    pub basmann_p_value: f64,
    pub df: usize,
    pub df_denom: usize,
}

/// IV:2SLS 过度识别检验（Stata estat overid）
/// - 同方差：Sargan, Basmann
/// - 稳健 VCE：Wooldridge (1995) robust score test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iv2slsOveridTest {
    /// "sargan_basmann" | "wooldridge"
    pub test_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sargan_stat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sargan_p_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basmann_stat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basmann_p_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wooldridge_stat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wooldridge_p_value: Option<f64>,
    pub df: usize,
}

/// IV:2SLS estat firststage 汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iv2slsFirstStageSummary {
    pub k_included_instruments: usize,
    pub k_excluded_instruments: usize,
    pub k_endogenous_regressors: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r2_adjusted: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_r2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f_stat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f_p_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f_df1: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f_df2: Option<usize>,
    pub shea_partial_r2: Vec<f64>,
    pub shea_adj_partial_r2: Vec<f64>,
    pub min_eigenvalue: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_eigenvalue_cv: Option<Iv2slsStockYogoCv>,
    /// "robust" | "k_endog_gt_2" when cv is None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_eigenvalue_cv_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iv2slsStockYogoBiasRow {
    pub pct_5: f64,
    pub pct_10: f64,
    pub pct_20: f64,
    pub pct_30: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iv2slsStockYogoSizeRow {
    pub pct_10: f64,
    pub pct_15: f64,
    pub pct_20: f64,
    pub pct_25: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iv2slsStockYogoCv {
    pub bias: Option<Iv2slsStockYogoBiasRow>,
    pub size: Iv2slsStockYogoSizeRow,
}

/// IV:2SLS 第一阶段单方程结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iv2slsFirstStageResult {
    pub endog_name: String,
    pub var_names: Vec<String>,
    pub coefficients: Vec<Coefficient>,
    pub r_squared: f64,
    pub adj_r_squared: f64,
}
