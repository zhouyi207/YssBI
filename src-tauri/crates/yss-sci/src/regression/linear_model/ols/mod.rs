//! Ordinary least squares: one configuration and one fitted result.
mod fit;
mod inference;
use faer::{Col, Mat};
use yss_sci_contract::regression::OlsOptions;

#[derive(Debug, Clone, PartialEq)]
pub enum OlsFitError {
    NotPositiveDefinite,
    Covariance(String),
    Inference(String),
}

impl std::fmt::Display for OlsFitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPositiveDefinite => formatter.write_str("OLS: X'X matrix is not positive definite (likely rank-deficient or has multicollinearity). Check your input variables."),
            Self::Covariance(message) | Self::Inference(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for OlsFitError {}

impl From<OlsFitError> for String {
    fn from(error: OlsFitError) -> Self {
        error.to_string()
    }
}

pub struct OLS {
    pub endog: Col<f64>, // 因变量 y
    pub exog: Mat<f64>,  // 自变量 X (n × k)
    pub config: OlsOptions,
}

#[derive(Debug)]
pub struct OlsFit {
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
    pub fvalue: f64,
    pub f_p_value: f64,

    pub betas: Col<f64>,
    pub fitted: Col<f64>,
    pub residuals: Col<f64>,
    pub rank: usize,
    pub stds: Col<f64>,
    pub tvalues: Col<f64>,
    pub pvalues: Col<f64>,
    pub conf_int_left: Col<f64>,  // 置信区间左侧
    pub conf_int_right: Col<f64>, // 置信区间右侧

    /// 参数协方差矩阵 (k×k)，用于 Wald 假设检验
    pub cov_beta: Mat<f64>,

    /// Nonrobust VCE: σ² (X'X)⁻¹, always available for Hausman test
    pub cov_beta_nonrobust: Mat<f64>,

    // 矩阵是否病态（多重共线性）
    pub cond_no: f64,
}
