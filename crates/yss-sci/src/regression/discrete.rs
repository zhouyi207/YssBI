use yss_sci_linalg::{Col, Mat};

pub mod fit;
pub mod logit;
pub mod probit;

/// Maximum-likelihood fit shared by the logit and probit estimators.
#[derive(Debug)]
pub struct BinaryFitResult {
    pub num_observation: usize,
    pub betas: Col<f64>,
    pub stds: Col<f64>,
    pub zvalues: Col<f64>,
    pub pvalues: Col<f64>,
    pub conf_int_left: Col<f64>,
    pub conf_int_right: Col<f64>,
    pub cov_beta: Mat<f64>,
    pub log_likelihood: f64,
    pub ll_null: f64,
    pub pseudo_r2: f64,
    pub lr_chi2: f64,
    pub lr_p_value: f64,
    pub aic: f64,
    pub bic: f64,
    pub iterations: usize,
    pub converged: bool,
}

pub use logit::*;
pub use probit::*;

pub mod postestimation;
