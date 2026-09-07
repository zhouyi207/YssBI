//! Typed OLS summary data. Rendering and label construction belong to the runtime.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegressionCoefficient {
    pub variable: String,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OlsModelSummary {
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OlsDiagnostics {
    pub cond_no: f64,
    pub fitted_values: Vec<f64>,
    pub residuals: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OlsSummary {
    pub title: String,
    pub endog_name: String,
    pub model_basic_info: OlsModelSummary,
    pub coefficients: Vec<RegressionCoefficient>,
    pub diagnostic_info: OlsDiagnostics,
    pub betas: Vec<f64>,
    pub cov_beta: Vec<Vec<f64>>,
}
