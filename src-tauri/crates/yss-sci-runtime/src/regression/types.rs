use crate::diagnostics::types::DiagnosticInfo;
use serde::{Deserialize, Serialize};
use yss_sci_contract::CategoricalRole;

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
