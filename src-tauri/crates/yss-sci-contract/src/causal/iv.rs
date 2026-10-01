//! Instrumental-variable estimator selection and numerical fit.
use crate::regression::{OlsOptions, fit::RegressionCoefficientStatistics};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentalVariableKind {
    TwoStageLeastSquares,
    LimitedInformationMaximumLikelihood,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentalVariableFit {
    pub family: String,
    pub response_name: String,
    pub parameter_names: Vec<String>,
    pub instrument_names: Vec<String>,
    pub options: OlsOptions,
    pub small: bool,
    pub coefficients: Vec<f64>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub inference: RegressionCoefficientStatistics,
    pub statistics: InstrumentalVariableStatistics,
    pub design: InstrumentalVariableDesign,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentalVariableStatistics {
    pub covariance_type: String,
    pub wald_chi2: f64,
    pub wald_p_value: f64,
    pub observations: usize,
    pub df_residual: usize,
    pub r2: f64,
    pub adjusted_r2: f64,
    pub condition_number: f64,
    /// The k-class parameter is 1 for 2SLS and estimated for LIML.
    pub kappa: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentalVariableDesign {
    pub exogenous: Vec<Vec<f64>>,
    pub endogenous: Vec<Vec<f64>>,
    pub instruments: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Copy)]
pub struct IvSummaryOptions {
    pub model_summary: bool,
    pub coefficient_table: bool,
    pub first_stage: bool,
    pub overidentification: bool,
    pub endogeneity: bool,
}

impl Default for IvSummaryOptions {
    fn default() -> Self {
        Self {
            model_summary: true,
            coefficient_table: true,
            first_stage: false,
            overidentification: false,
            endogeneity: false,
        }
    }
}

/// estat firststage 汇总（Stata estat firststage）
/// - 单内生：R², Adj R², Partial R², F, Prob>F, Min eigenvalue
/// - 多内生：Shea partial R², Shea adj partial R², Min eigenvalue
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct StockYogoBiasRow {
    pub pct_5: f64,
    pub pct_10: f64,
    pub pct_20: f64,
    pub pct_30: f64,
}

/// Stock-Yogo 2SLS size of nominal 5% Wald test 临界值（10%, 15%, 20%, 25%）
#[derive(Debug, Clone, Serialize)]
pub struct StockYogoSizeRow {
    pub pct_10: f64,
    pub pct_15: f64,
    pub pct_20: f64,
    pub pct_25: f64,
}

/// Stock-Yogo 弱工具变量临界值（Stata 表）
#[derive(Debug, Clone, Serialize)]
pub struct StockYogoCriticalValues {
    pub bias: Option<StockYogoBiasRow>,
    pub size: StockYogoSizeRow,
}

/// 第一阶段单方程结果（每个内生变量对 Z = [exog, instruments] 的回归）
#[derive(Debug, Clone, Serialize)]
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

/// 豪斯曼检验结果（Stata hausman iv ols, constant sigmamore）
/// 仅当 nonrobust VCE 时计算。H0: 内生变量可视为外生（OLS 与 IV 一致）
#[derive(Debug, Clone, Serialize)]
pub struct HausmanTest {
    pub stat: f64,
    pub p_value: f64,
    pub df: usize,
}

/// 内生性检验结果（Stata estat endogenous）
/// Durbin (1954) score test 与 Wu-Hausman (Wu 1974; Hausman 1978) test
/// 仅当 nonrobust VCE 时计算。H0: 被检内生变量可视为外生
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
/// LIML overidentification test (Stata estat overid)
/// Anderson-Rubin (1950) chi2, Basmann F
#[derive(Debug, Clone, Serialize)]
pub struct LimlOveridTest {
    pub anderson_rubin_stat: f64,
    pub anderson_rubin_p_value: f64,
    pub basmann_stat: f64,
    pub basmann_p_value: f64,
    pub df: usize,
    pub df_denom: usize,
}
