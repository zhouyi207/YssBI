//! Bounded coefficient reads share Summary permissions and the original fitted values.
use super::{ApplicationState, LinearSummaryOptions, ReportQueryError, ResultReference};
pub use yss_sci_contract::regression::report::RegressionCoefficient;

pub struct LinearCoefficientPage {
    pub offset: usize,
    pub total_count: usize,
    pub coefficients: Vec<RegressionCoefficient>,
}

impl ApplicationState {
    pub fn query_result_coefficients(
        &self,
        reference: ResultReference,
        offset: usize,
        limit: usize,
    ) -> Result<LinearCoefficientPage, ReportQueryError> {
        self.with_linear_regression_summary(reference, |result, summary| {
            if !selected(&summary.options) {
                return Err(ReportQueryError::InvalidRequest);
            }
            page(&result.report.coefficients, offset, limit)
        })
    }
}

pub(super) fn selected(options: &LinearSummaryOptions) -> bool {
    options.equation || options.coefficient_table || options.coefficient_chart
}

pub(super) fn page(
    coefficients: &[RegressionCoefficient],
    offset: usize,
    limit: usize,
) -> Result<LinearCoefficientPage, ReportQueryError> {
    let range = super::page_range(coefficients.len(), offset, limit)?;
    let rows = &coefficients[range.clone()];
    if rows.iter().any(|row| {
        [
            row.coef,
            row.std_err,
            row.t_value,
            row.p_value,
            row.ci_lower,
            row.ci_upper,
        ]
        .iter()
        .any(|value| !value.is_finite())
    }) {
        return Err(ReportQueryError::UnrepresentableValue);
    }
    Ok(LinearCoefficientPage {
        offset: range.start,
        total_count: coefficients.len(),
        coefficients: rows.to_vec(),
    })
}
