//! Scientific regression report and transport-neutral model structures.

use serde::{Deserialize, Serialize};
use yss_sci_contract::CategoricalRole;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionCoefficientStatistics {
    pub covariance: Vec<Vec<f64>>,
    pub standard_errors: Vec<f64>,
    pub statistic_values: Vec<f64>,
    pub p_values: Vec<f64>,
    pub confidence_interval_lower: Vec<f64>,
    pub confidence_interval_upper: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinearRegressionStatistics {
    pub r2: f64,
    pub adjusted_r2: f64,
    pub f_statistic: f64,
    pub f_p_value: f64,
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
    pub condition_number: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BinaryRegressionLink {
    Logit,
    Probit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryRegressionStatistics {
    pub log_likelihood: f64,
    pub null_log_likelihood: f64,
    pub pseudo_r2: f64,
    pub adjusted_pseudo_r2: f64,
    pub lr_chi2: f64,
    pub lr_p_value: f64,
    pub aic: f64,
    pub bic: f64,
    pub iterations: usize,
    pub converged: bool,
    pub condition_number: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PraisRegressionStatistics {
    #[serde(flatten)]
    pub linear: LinearRegressionStatistics,
    pub rho: f64,
    pub durbin_watson_original: f64,
    pub durbin_watson_transformed: f64,
    pub iterations: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RegressionStatistics {
    Linear {
        #[serde(flatten)]
        coefficients: RegressionCoefficientStatistics,
        #[serde(flatten)]
        model: LinearRegressionStatistics,
    },
    Binary {
        link: BinaryRegressionLink,
        #[serde(flatten)]
        coefficients: RegressionCoefficientStatistics,
        #[serde(flatten)]
        model: BinaryRegressionStatistics,
    },
    Prais {
        #[serde(flatten)]
        coefficients: RegressionCoefficientStatistics,
        #[serde(flatten)]
        model: PraisRegressionStatistics,
    },
}

impl RegressionStatistics {
    pub fn coefficient_statistics(&self) -> &RegressionCoefficientStatistics {
        match self {
            Self::Linear { coefficients, .. }
            | Self::Binary { coefficients, .. }
            | Self::Prais { coefficients, .. } => coefficients,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum VariableSpec {
    Numeric {
        name: String,
    },
    Categorical {
        name: String,
        categories: Vec<String>,
        dropped: String,
        role: CategoricalRole,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept_indices: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogitConfigure {
    pub constant: bool,
}

impl Default for LogitConfigure {
    fn default() -> Self {
        Self { constant: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogitModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbitConfigure {
    pub constant: bool,
}

impl Default for ProbitConfigure {
    fn default() -> Self {
        Self { constant: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbitModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
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
    /// Nonrobust VCE for Hausman test (panel models only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cov_beta_nonrobust: Option<Vec<Vec<f64>>>,
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
    /// For IV:2SLS, Wald chi2 and prob (asymptotic inference). OLS/Prais/WLS/GLS use F; set to None.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wald_chi2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prob_wald_chi2: Option<f64>,
    /// MLE: log likelihood (Stata xtreg, mle)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_likelihood: Option<f64>,
    /// MLE: LR chi2 (Stata xtreg, mle)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lr_chi2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prob_lr_chi2: Option<f64>,
    /// MLE: chibar2(01) for sigma_u=0 test
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chibar2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prob_chibar2: Option<f64>,
    /// MLE: constant-only model iterations (Stata "Fitting constant-only model")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mle_iter_log_lik_const: Option<Vec<f64>>,
    /// MLE: full model iterations (Stata "Fitting full model")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mle_iter_log_lik: Option<Vec<f64>>,
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

/// Observations per group (min/avg/max)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsPerGroupInfo {
    pub min: usize,
    pub avg: f64,
    pub max: usize,
}

/// Variance decomposition: σ_u, σ_e, ρ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigmaInfo {
    pub sigma_u: f64,
    pub sigma_e: f64,
    pub rho: f64,
}

/// RE quasi-demeaning parameter θ (min/avg/max across groups)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThetaInfo {
    pub min: f64,
    pub avg: f64,
    pub max: f64,
}

/// Panel FE-specific stats (Stata xtreg, fe)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct PanelFEInfo {
    /// R² Within/Between/Overall. None for MLE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r2_within: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r2_between: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r2_overall: Option<f64>,
    pub num_groups: usize,
    pub obs_per_group: ObsPerGroupInfo,
    pub sigma: SigmaInfo,
    pub corr_u_i_Xb: f64,
    /// RE quasi-demeaning parameter θ (min/avg/max across groups)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theta: Option<ThetaInfo>,
    /// MLE: chibar2(01) for sigma_u=0 (Stata xtreg, mle)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chibar2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prob_chibar2: Option<f64>,
}

/// Classification table for binary choice models (Stata estat classification)
/// Rows: Classified + (pred≥cutoff), Classified - (pred<cutoff)
/// Cols: True D (y=1), True ~D (y=0), Total
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationTable {
    /// True positives: classified +, actual D
    pub tp: usize,
    /// False positives: classified +, actual ~D
    pub fp: usize,
    /// False negatives: classified -, actual D
    pub fn_: usize,
    /// True negatives: classified -, actual ~D
    pub tn: usize,
    /// Cutoff used (default 0.5)
    pub cutoff: f64,
    /// Sensitivity Pr(+|D) = TP/(TP+FN)
    pub sensitivity: f64,
    /// Specificity Pr(-|~D) = TN/(TN+FP)
    pub specificity: f64,
    /// Positive predictive value Pr(D|+)
    pub ppv: f64,
    /// Negative predictive value Pr(~D|-)
    pub npv: f64,
    /// False + rate for true ~D Pr(+|~D)
    pub false_pos_rate: f64,
    /// False - rate for true D Pr(-|D)
    pub false_neg_rate: f64,
    /// Percent correctly classified
    pub pct_correct: f64,
}

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

#[cfg(test)]
mod tests {
    use crate::panel::did::{
        ComputeDidFakeGroupRequest, DidFakeGroupEnginePayload, ExogLabelEntry,
    };
    use crate::regression::types::{OLSModel, OLSResult};
    use serde_json::json;

    #[test]
    fn regression_models_preserve_wire_shape() {
        let model = OLSModel {
            betas: vec![1.0, 2.5],
            has_constant: true,
            variable_specs: vec![],
            kept_indices: Some(vec![0, 2]),
        };
        assert_eq!(
            serde_json::to_value(model).unwrap(),
            json!({
                "betas": [1.0, 2.5],
                "has_constant": true,
                "variable_specs": [],
                "kept_indices": [0, 2]
            })
        );

        let result: OLSResult = serde_json::from_value(json!({
            "title": "OLS",
            "endog_name": "y",
            "model_basic_info": {
                "model_type": "OLS",
                "method": "Least Squares",
                "num_observation": 2,
                "r_squared": 1.0,
                "adj_r_squared": 1.0,
                "f_statistic": 0.0,
                "prob_f_statistic": 1.0,
                "df_model": 1,
                "df_residual": 1,
                "df_total": 2,
                "ss_model": 1.0,
                "ss_residual": 0.0,
                "ss_total": 1.0,
                "ms_model": 1.0,
                "ms_residual": 0.0,
                "ms_total": 0.5,
                "covariance_type": "nonrobust",
                "aic": 0.0,
                "bic": 0.0
            },
            "coefficients": [],
            "diagnostic_info": { "cond_no": 1.0 },
            "betas": [1.0],
            "cov_beta": [[0.25]]
        }))
        .unwrap();
        let serialized = serde_json::to_value(result).unwrap();
        assert_eq!(serialized["title"], "OLS");
        assert_eq!(serialized["cov_beta"], json!([[0.25]]));
        assert!(serialized.get("cov_beta_nonrobust").is_none());
    }

    #[test]
    fn panel_did_request_uses_the_existing_flattened_wire_shape() {
        let request = ComputeDidFakeGroupRequest {
            payload: DidFakeGroupEnginePayload {
                endog: vec![1.0],
                exog_row_major: vec![1.0],
                ncols: 1,
                all_labels: vec![ExogLabelEntry {
                    variable: "did".to_string(),
                    category: None,
                }],
                entity_id: vec![0],
                time_id: vec![0],
                post: vec![1.0],
                treat: vec![1.0],
                did_label: "did".to_string(),
                observed_coef: 1.0,
                constant: true,
                cov_type: "cluster".to_string(),
            },
            n_perm: 99,
            rng_seed: 7,
        };
        let serialized = serde_json::to_value(request).unwrap();
        assert_eq!(serialized["did_label"], "did");
        assert_eq!(serialized["n_perm"], 99);
        assert_eq!(serialized["rng_seed"], 7);
        assert!(serialized.get("payload").is_none());
    }

    #[test]
    fn scientific_models_do_not_import_node_identity_layers() {
        let model_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let forbidden = [
            ["node", "_system"].concat(),
            ["graph", "::register"].concat(),
            ["Node", "Definition"].concat(),
            ["Pin", "Definition"].concat(),
            ["Pin", "Role"].concat(),
            ["NodeInstance", "Params"].concat(),
        ];
        let mut offenders = Vec::new();
        for name in ["regression/types.rs", "panel/did.rs"] {
            let source = std::fs::read_to_string(model_root.join(name)).unwrap_or_else(|error| {
                panic!("scientific model {name} must be readable: {error}")
            });
            for (line_index, line) in source.lines().enumerate() {
                for pattern in &forbidden {
                    if line.contains(pattern) {
                        offenders.push(format!("{name}:{}:{pattern}", line_index + 1));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "scientific model identity dependencies:\n{}",
            offenders.join("\n")
        );
    }
}
