//! Compact equation fits; full observation arrays are emitted as relations by each adapter.
use yss_sci_contract::regression::models::{
    ModelStatistics, RegressionCoefficient, RegressionModelResult,
};
#[derive(serde::Serialize)]
pub(super) struct EquationSummary<'a> {
    coefficients: &'a [RegressionCoefficient],
    covariance: &'a Option<Vec<Vec<f64>>>,
    statistics: &'a ModelStatistics,
}
pub(super) fn equation(model: &RegressionModelResult) -> EquationSummary<'_> {
    EquationSummary {
        coefficients: &model.coefficients,
        covariance: &model.covariance,
        statistics: &model.statistics,
    }
}
