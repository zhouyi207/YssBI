//! 信息展示相关的数据结构

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSResult {
    pub title: String,
    pub endog_name: String,
    pub model_basic_info: ModelBasicInfo,
    pub coefficients: Vec<Coefficient>,
    pub diagnostic_info: DiagnosticInfo,
    /// 参数估计 (与 coefficients 的 coef 一致)，用于假设检验
    pub betas: Vec<f64>,
    /// 参数协方差矩阵 (k×k)，行优先，用于假设检验
    pub cov_beta: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBasicInfo {
    pub model_type: String,
    pub method: String,
    pub num_observation: usize,
    pub r_squared: f64,
    pub adj_r_squared: f64,
    pub f_statistic: f64,
    pub prob_f_statistic: f64,
    pub df_model: usize,
    pub df_residual: usize,
    pub df_total: usize,
    pub ss_model: f64,
    pub ss_residual: f64,
    pub ss_total: f64,
    pub ms_model: f64,
    pub ms_residual: f64,
    pub ms_total: f64,
    pub covariance_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coefficient {
    pub variable: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub coef: f64,
    pub std_err: f64,
    pub t_value: f64,
    pub p_value: f64,
    #[serde(rename = "confidence_interval_0.025")]
    pub ci_lower: f64,
    #[serde(rename = "confidence_interval_0.975")]
    pub ci_upper: f64,
    pub is_significant: bool,
}

/// Breusch-Pagan 异方差检验结果（单变体）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreuschPaganTest {
    pub lm_stat: f64,
    pub df: usize,
    pub p_value: f64,
}

/// Breusch-Pagan 四种变体（对应 Stata estat hettest）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreuschPaganTests {
    /// estat hettest（z=拟合值，原始 BP）
    pub stata: Option<BreuschPaganTest>,
    /// estat hettest, iid（z=拟合值，Koenker）
    pub koenker: Option<BreuschPaganTest>,
    /// estat hettest, rhs（z=RHS，原始 BP）
    pub stata_rhs: Option<BreuschPaganTest>,
    /// estat hettest, rhs iid（z=RHS，Koenker）
    pub koenker_rhs: Option<BreuschPaganTest>,
}

/// IM-test 各分量的 chi² 检验结果（chi2, df, p）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImTestComponent {
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// Cameron & Trivedi (1990) IM-test 分解（estat imtest）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImTest {
    pub heteroskedasticity: ImTestComponent,
    pub skewness: ImTestComponent,
    pub kurtosis: ImTestComponent,
    pub total: ImTestComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticInfo {
    pub cond_no: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bp_tests: Option<BreuschPaganTests>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub im_test: Option<ImTest>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fitted_values: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub residuals: Vec<f64>,
}
