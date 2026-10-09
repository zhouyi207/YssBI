//! Numerical IV input shared by 2SLS, LIML and postestimation.
use yss_sci_contract::regression::OlsOptions;
use yss_sci_linalg::{Col, Mat};

pub struct IvModel {
    pub endog: Col<f64>,
    pub exog: Mat<f64>,
    pub endog_reg: Mat<f64>,
    pub instruments: Mat<f64>,
    pub options: OlsOptions,
    /// Use residual degrees for finite-sample covariance and inference.
    pub small: bool,
}
