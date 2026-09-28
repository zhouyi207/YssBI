use crate::causal::iv::types::{
    Iv2slsEndogenousTest, Iv2slsFirstStageResult, Iv2slsFirstStageSummary, Iv2slsHausmanTest,
    Iv2slsOveridDims, Iv2slsOveridTest, IvLimlOveridTest,
};
use crate::panel::types::PanelFEInfo;
use crate::regression::discrete::types::ClassificationTable;
use crate::regression::linear::types::PraisInfo;
use serde::{Deserialize, Serialize};

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

/// VIF 多重共线性检验单变量结果（对应 Stata estat vif）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VifEntry {
    pub variable: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub vif: f64,
    pub tolerance: f64, // 1/VIF
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
pub struct LeverageKdePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticInfo {
    pub cond_no: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vif: Option<Vec<VifEntry>>,
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
    /// Leverage（帽子矩阵对角元，Stata predict lev, leverage）
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub leverage: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub leverage_kde: Vec<LeverageKdePoint>,
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
    /// IV:2SLS 第一阶段回归结果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iv2sls_first_stage: Option<Vec<Iv2slsFirstStageResult>>,
    /// IV:2SLS estat firststage 汇总（单内生/多内生，robust/nonrobust）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iv2sls_first_stage_summary: Option<Iv2slsFirstStageSummary>,
    /// IV:2SLS 过度识别检验（estat overid）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iv2sls_overid: Option<Iv2slsOveridTest>,
    /// IV:2SLS 过度识别维度（k_iv=排除的工具变量数, k_endog=内生数），用于诊断
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iv2sls_overid_dims: Option<Iv2slsOveridDims>,
    /// IV:2SLS 传统豪斯曼检验（hausman iv ols, constant sigmamore），仅 nonrobust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iv2sls_hausman: Option<Iv2slsHausmanTest>,
    /// IV:2SLS Durbin-Wu-Hausman 内生性检验（estat endogenous），仅 nonrobust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iv2sls_endogenous: Option<Iv2slsEndogenousTest>,
    /// IV:LIML κ (minimum eigenvalue)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ivliml_kappa: Option<f64>,
    /// IV:LIML 过度识别检验（estat overid）Anderson-Rubin, Basmann F
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ivliml_overid: Option<IvLimlOveridTest>,
    /// Binary choice (Logit/Probit): classification table (Stata estat classification)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_table: Option<ClassificationTable>,
    /// Binary choice: mean of each exog column (for margins at means)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exog_means: Option<Vec<f64>>,
    /// Panel FE: Stata xtreg, fe style (R2 Within/Between/Overall, sigma_u, sigma_e, rho, corr, obs per group)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panel_fe_info: Option<PanelFEInfo>,
    /// Variables omitted due to strict multicollinearity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub omit_info: Option<OmitInfo>,
}

/// Variables omitted due to strict multicollinearity (Stata-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmitInfo {
    pub omitted: Vec<OmittedVariable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmittedVariable {
    pub variable: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub reason: String,
}
