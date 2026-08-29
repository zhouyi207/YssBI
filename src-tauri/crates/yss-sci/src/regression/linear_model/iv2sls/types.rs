//! IV:2SLS (Instrumental Variables Two-Stage Least Squares)
//!
//! Stata ivregress 2sls: depvar [varlist1] (varlist2 = varlistiv)
//! - varlist1: exogenous variables (in both stages)
//! - varlist2: endogenous variables (instrumented in stage 1)
//! - varlistiv: instruments (stage 1 only)
//!
//! Stage 1: Regress each endogenous on Z = [exog, instruments] → endog_hat
//! Stage 2: Regress Y on X = [exog, endog_hat] → β. VCE uses structural residuals u = y - X_struct*β.

use crate::regression::covariance::CovParams;
use ndarray::{Array1, Array2};

/// 2SLS 配置，与 OLS 一致（constant, cov_type, cov_params）
pub struct IV2SLSConfig {
    pub constant: bool,
    pub cov_type: String,
    pub cov_params: Option<CovParams>,
    /// Stata small: if true, use ESS/(n-k) for σ²; if false, use ESS/n (Stata default).
    pub small: bool,
}

/// IV:2SLS 输入
/// - endog: y (n,)
/// - exog: exogenous variables (n × k_exog)，不含 constant
/// - endog_reg: endogenous variables (n × k_endog)
/// - instruments: instruments (n × k_iv)
///
/// 识别条件: k_iv >= k_endog
pub struct IV2SLS {
    pub endog: Array1<f64>,
    pub exog: Array2<f64>,
    pub endog_reg: Array2<f64>,
    pub instruments: Array2<f64>,
    pub config: IV2SLSConfig,
    /// 内生变量名称，用于 first_stage 输出
    pub endog_names: Option<Vec<String>>,
    /// Z 矩阵变量名 [const?, exog..., instruments...]，用于 first_stage 系数标签
    pub z_var_names: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct IV2SLSModel {
    pub params: Array1<f64>,
}

/// estat firststage 汇总（Stata estat firststage）
/// - 单内生：R², Adj R², Partial R², F, Prob>F, Min eigenvalue
/// - 多内生：Shea partial R², Shea adj partial R², Min eigenvalue
#[derive(Debug, Clone)]
pub struct FirstStageSummary {
    /// Included instruments (X1 excluding constant): exogenous regressors in both structural and first stage
    pub k_included_instruments: usize,
    /// Excluded instruments (X2): only in first stage
    pub k_excluded_instruments: usize,
    /// Endogenous regressors (instrumented)
    pub k_endogenous_regressors: usize,
    /// 单内生时为 Some；多内生时为 None
    pub r2: Option<f64>,
    pub r2_adjusted: Option<f64>,
    pub partial_r2: Option<f64>,
    pub f_stat: Option<f64>,
    pub f_p_value: Option<f64>,
    pub f_df1: Option<usize>,
    pub f_df2: Option<usize>,
    /// 多内生时每变量一个
    pub shea_partial_r2: Vec<f64>,
    pub shea_adj_partial_r2: Vec<f64>,
    pub min_eigenvalue: f64,
    /// 仅 nonrobust 且 k_endog<=2 时提供；否则 None
    pub min_eigenvalue_cv: Option<StockYogoCriticalValues>,
    /// 当 min_eigenvalue_cv 为 None 时的原因："robust" | "k_endog_gt_2"
    pub min_eigenvalue_cv_note: Option<String>,
}

/// Stock-Yogo 2SLS relative bias 临界值（5%, 10%, 20%, 30%）
/// 在 k2 < k1+2 时 Stock-Yogo 未提供，整行为 None
#[derive(Debug, Clone)]
pub struct StockYogoBiasRow {
    pub pct_5: f64,
    pub pct_10: f64,
    pub pct_20: f64,
    pub pct_30: f64,
}

/// Stock-Yogo 2SLS size of nominal 5% Wald test 临界值（10%, 15%, 20%, 25%）
#[derive(Debug, Clone)]
pub struct StockYogoSizeRow {
    pub pct_10: f64,
    pub pct_15: f64,
    pub pct_20: f64,
    pub pct_25: f64,
}

/// Stock-Yogo 弱工具变量临界值（Stata 表）
#[derive(Debug, Clone)]
pub struct StockYogoCriticalValues {
    pub bias: Option<StockYogoBiasRow>,
    pub size: StockYogoSizeRow,
}

/// 第一阶段单方程结果（每个内生变量对 Z = [exog, instruments] 的回归）
#[derive(Debug, Clone)]
pub struct FirstStageResult {
    pub endog_name: String,
    /// 自变量名称（const, exog..., instruments...）
    pub var_names: Vec<String>,
    pub betas: Vec<f64>,
    pub stds: Vec<f64>,
    pub tvalues: Vec<f64>,
    pub pvalues: Vec<f64>,
    pub conf_int_left: Vec<f64>,
    pub conf_int_right: Vec<f64>,
    pub r2: f64,
    pub r2_adjusted: f64,
}

#[derive(Debug)]
pub struct IV2SLSResult {
    pub num_observation: usize,
    pub ss_model: f64,
    pub ss_residual: f64,
    pub ss_total: f64,
    pub df_model: usize,
    pub df_residual: usize,
    pub df_total: usize,
    pub ms_model: f64,
    pub ms_residual: f64,
    pub ms_total: f64,
    pub covariance_type: String,
    pub r2: f64,
    pub r2_adjusted: f64,
    /// Wald chi2 for joint significance (2SLS uses asymptotic inference, not F)
    pub wald_chi2: f64,
    pub wald_chi2_p_value: f64,

    pub model: IV2SLSModel,
    pub betas: Array1<f64>,
    pub stds: Array1<f64>,
    /// z-statistics (2SLS uses asymptotic normal inference, not t)
    pub zvalues: Array1<f64>,
    pub pvalues: Array1<f64>,
    pub conf_int_left: Array1<f64>,
    pub conf_int_right: Array1<f64>,
    pub cov_beta: Array2<f64>,
    pub cond_no: f64,

    /// 第一阶段回归结果（每个内生变量）
    pub first_stage: Vec<FirstStageResult>,
    /// estat firststage 汇总
    pub first_stage_summary: FirstStageSummary,

    /// 过度识别检验（estat overid），仅当 k_iv > k_endog 时计算（排除的工具变量数 > 内生变量数）
    pub overid: Option<OveridTest>,
    /// 用于诊断：k_iv = 排除的工具变量数，k_endog = 内生变量数
    pub overid_k_iv: usize,
    pub overid_k_endog: usize,

    /// 传统豪斯曼检验（hausman iv ols, constant sigmamore），仅 nonrobust VCE
    pub hausman: Option<HausmanTest>,
    /// Durbin-Wu-Hausman 内生性检验（estat endogenous），仅 nonrobust VCE
    pub endogenous: Option<EndogenousTest>,
}

/// 豪斯曼检验结果（Stata hausman iv ols, constant sigmamore）
/// 仅当 nonrobust VCE 时计算。H0: 内生变量可视为外生（OLS 与 IV 一致）
#[derive(Debug, Clone)]
pub struct HausmanTest {
    pub stat: f64,
    pub p_value: f64,
    pub df: usize,
}

/// 内生性检验结果（Stata estat endogenous）
/// Durbin (1954) score test 与 Wu-Hausman (Wu 1974; Hausman 1978) test
/// 仅当 nonrobust VCE 时计算。H0: 被检内生变量可视为外生
#[derive(Debug, Clone)]
pub struct EndogenousTest {
    pub durbin_stat: f64,
    pub durbin_p_value: f64,
    pub wu_stat: f64,
    pub wu_p_value: f64,
    pub df: usize,
    pub wu_df_denom: usize,
}

/// 过度识别检验结果（Stata estat overid）
/// - 同方差（nonrobust）：Sargan、Basmann
/// - 稳健 VCE（HC0/HC1/HC2/HC3/cluster/HAC/newey）：Wooldridge (1995) robust score test
#[derive(Debug, Clone)]
pub struct OveridTest {
    /// "sargan_basmann" | "wooldridge"
    pub test_type: String,
    /// Sargan/Basmann（同方差时有效）
    pub sargan_stat: Option<f64>,
    pub sargan_p_value: Option<f64>,
    pub basmann_stat: Option<f64>,
    pub basmann_p_value: Option<f64>,
    /// Wooldridge score（稳健 VCE 时有效，Stata estat overid）
    pub wooldridge_stat: Option<f64>,
    pub wooldridge_p_value: Option<f64>,
    pub df: usize,
}
