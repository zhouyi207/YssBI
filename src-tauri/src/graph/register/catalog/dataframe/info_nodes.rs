//! 信息展示相关的数据结构

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// 计算 OLS/WLS/GLS 的 AIC 和 BIC（与 Stata estat ic 一致）
/// 公式: ll = -n/2 * (ln(2π) + ln(σ²) + 1), σ² = ss_residual/n (MLE)
/// AIC = -2*ll + 2*k, BIC = -2*ll + k*ln(n)
/// 注意: ln(2π) 而非 π*ln(2)
pub fn compute_aic_bic(n: usize, k: usize, ss_residual: f64) -> (f64, f64) {
    let n_f = n as f64;
    let k_f = k as f64;
    let sigma2 = if n > 0 && ss_residual >= 0.0 {
        (ss_residual / n_f).max(1e-300)
    } else {
        1e-300
    };
    let ln_2pi = (2.0 * PI).ln();
    let llf = -n_f / 2.0 * (ln_2pi + sigma2.ln() + 1.0);
    let aic = -2.0 * llf + 2.0 * k_f;
    let bic = -2.0 * llf + k_f * n_f.ln();
    (aic, bic)
}

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
    pub aic: f64,
    pub bic: f64,
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

/// Ramsey RESET 检验单变体（F 检验）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvTest {
    pub f_stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

/// Ramsey RESET 两种变体（对应 Stata estat ovtest）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvTests {
    /// estat ovtest（z=拟合值幂 ŷ²,ŷ³,ŷ⁴）
    pub default: Option<OvTest>,
    /// estat ovtest, rhs（z=RHS 变量幂）
    pub rhs: Option<OvTest>,
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

/// 残差正态性检验（Omnibus / Jarque-Bera，statsmodels 风格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalityTests {
    pub skewness: f64,
    pub kurtosis: f64,
    pub omnibus_stat: f64,
    pub omnibus_p_value: f64,
    pub jarque_bera_stat: f64,
    pub jarque_bera_p_value: f64,
}

/// 各诊断模块的后端计算耗时（毫秒），用于性能分析
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiagnosticTiming {
    /// 拟合值与残差计算
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fitted_residuals_ms: Option<u64>,
    /// Breusch-Pagan 异方差检验
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bp_tests_ms: Option<u64>,
    /// Ramsey RESET 检验
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ov_tests_ms: Option<u64>,
    /// Cameron & Trivedi IM-test
    #[serde(skip_serializing_if = "Option::is_none")]
    pub im_test_ms: Option<u64>,
}

/// 残差 vs 残差滞后 1 的散点图数据（用于自相关诊断）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidualScatterData {
    /// e_t（当前残差）
    pub e: Vec<f64>,
    /// e_{t-1}（滞后 1 残差）
    pub e_lag1: Vec<f64>,
    /// 可选：每个点对应的时间（用于 tooltip 等）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticInfo {
    pub cond_no: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bp_tests: Option<BreuschPaganTests>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ov_tests: Option<OvTests>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub im_test: Option<ImTest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normality_tests: Option<NormalityTests>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fitted_values: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub residuals: Vec<f64>,
    /// 残差 vs 残差滞后 1 散点图数据（e 与 e_lag1）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub residual_scatter: Option<ResidualScatterData>,
    /// 回归设计矩阵 X（行优先），用于 Breusch-Godfrey 检验
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exog: Option<Vec<Vec<f64>>>,
    /// 各诊断模块耗时（用于性能分析）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<DiagnosticTiming>,
    /// Prais 特有：ρ、原始 DW、变换后 DW
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prais_info: Option<PraisInfo>,
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
