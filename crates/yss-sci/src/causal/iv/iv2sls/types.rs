//! IV:2SLS (Instrumental Variables Two-Stage Least Squares)
//!
//! Stata ivregress 2sls: depvar [varlist1] (varlist2 = varlistiv)
//! - varlist1: exogenous variables (in both stages)
//! - varlist2: endogenous variables (instrumented in stage 1)
//! - varlistiv: instruments (stage 1 only)
//!
//! Stage 1: Regress each endogenous on Z = [exog, instruments] → endog_hat
//! Stage 2: Regress Y on X = [exog, endog_hat] → β. VCE uses structural residuals u = y - X_struct*β.

use yss_sci_contract::regression::CovParams;
use yss_sci_linalg::{Col, Mat};

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
    pub endog: Col<f64>,
    pub exog: Mat<f64>,
    pub endog_reg: Mat<f64>,
    pub instruments: Mat<f64>,
    pub config: IV2SLSConfig,
    /// 内生变量名称，用于 first_stage 输出
    pub endog_names: Option<Vec<String>>,
    /// Z 矩阵变量名 [const?, exog..., instruments...]，用于 first_stage 系数标签
    pub z_var_names: Option<Vec<String>>,
}

pub use yss_sci_contract::causal::iv::{
    EndogenousTest, FirstStageResult, FirstStageSummary, HausmanTest, OveridTest, StockYogoBiasRow,
    StockYogoCriticalValues, StockYogoSizeRow,
};
