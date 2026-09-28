//! Shared k-class fit facts for 2SLS and LIML.
use yss_sci_contract::causal::iv::InstrumentalVariableStatistics;
use yss_sci_contract::regression::fit::RegressionCoefficientStatistics;
use yss_sci_linalg::Col;

#[derive(Debug)]
pub struct IvEstimate {
    pub betas: Col<f64>,
    pub fitted: Col<f64>,
    pub residuals: Col<f64>,
    pub inference: RegressionCoefficientStatistics,
    pub statistics: InstrumentalVariableStatistics,
}
