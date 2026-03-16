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

/// Compute classification table for binary choice (Stata estat classification)
pub fn compute_classification_table(
    endog: &[f64],
    fitted: &[f64],
    cutoff: f64,
) -> ClassificationTable {
    let n = endog.len().min(fitted.len());
    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut fn_ = 0usize;
    let mut tn = 0usize;
    for i in 0..n {
        let y = endog[i];
        let pred_pos = fitted[i] >= cutoff;
        let actual_pos = y > 0.5;
        match (pred_pos, actual_pos) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, true) => fn_ += 1,
            (false, false) => tn += 1,
        }
    }
    let total_d = tp + fn_;
    let total_nd = tn + fp;
    let total_pos = tp + fp;
    let total_neg = tn + fn_;
    let sensitivity = if total_d > 0 { tp as f64 / total_d as f64 } else { 0.0 };
    let specificity = if total_nd > 0 { tn as f64 / total_nd as f64 } else { 0.0 };
    let ppv = if total_pos > 0 { tp as f64 / total_pos as f64 } else { 0.0 };
    let npv = if total_neg > 0 { tn as f64 / total_neg as f64 } else { 0.0 };
    let false_pos_rate = if total_nd > 0 { fp as f64 / total_nd as f64 } else { 0.0 };
    let false_neg_rate = if total_d > 0 { fn_ as f64 / total_d as f64 } else { 0.0 };
    let pct_correct = if n > 0 {
        (tp + tn) as f64 / n as f64 * 100.0
    } else {
        0.0
    };
    ClassificationTable {
        tp,
        fp,
        fn_,
        tn,
        cutoff,
        sensitivity,
        specificity,
        ppv,
        npv,
        false_pos_rate,
        false_neg_rate,
        pct_correct,
    }
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
