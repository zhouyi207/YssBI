//! Linear-model computation requests and results.
use super::OlsOptions;

#[derive(Debug, Clone, PartialEq)]
pub enum LinearRegressionMethod {
    Ols,
    Wls {
        weights: Vec<f64>,
    },
    /// Relative error covariance, in row-major order and fitted observation order.
    Gls {
        sigma: Vec<Vec<f64>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearRegressionRequest {
    pub response: Vec<f64>,
    pub predictors: Vec<Vec<f64>>,
    pub options: OlsOptions,
    pub method: LinearRegressionMethod,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearRegressionResult {
    pub constant: bool,
    pub coefficients: Vec<f64>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    /// Design columns in fitted parameter order; each column retains observation order.
    pub design: Vec<Vec<f64>>,
    pub report: crate::regression::report::LinearRegressionSummary,
}
