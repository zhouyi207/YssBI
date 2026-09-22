use thiserror::Error;
use yss_data_contract::TabularScalar;
use yss_graph_execution::result::ResultReference;
use yss_node_kernel::{LinearSummary, RuntimeValue};
use yss_relational_contract::RelationColumn;
use yss_sci_contract::regression::report::LinearModelSummary;
use yss_sci_contract::regression::summary::LinearSummaryOptions;
use yss_sci_contract::scientific::{AcfPacfResult, LinearRegressionResult};

use super::{MAX_RESULT_PAGE_ROWS, ResultPageKind, ResultPageProjection};
use crate::session::{ApplicationState, SessionCaptureError};
use yss_sci_contract::hypothesis::HypothesisTestOutput;
use yss_sci_contract::serial_tests::SerialTestsOutput;

pub(crate) mod presentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultTablePart {
    Coefficients,
    Observations,
}

pub struct LinearRegressionReportProjection {
    pub summary: LinearSummaryOptions,
    pub reference: ResultReference,
    pub title: String,
    pub endog_name: String,
    pub param_names: Vec<String>,
    pub model: LinearModelSummary,
    pub condition_number: f64,
    pub coefficient_count: usize,
    pub observation_count: usize,
}

pub enum ResultAnalysisRequest {
    ResidualPlot {
        max_points: usize,
        x_range: Option<[f64; 2]>,
    },
    AcfPacf,
    SerialTests,
    Hypothesis,
}

pub struct ResidualPlotPoint {
    pub observation: usize,
    pub x: f64,
    pub y: f64,
}

pub struct ResidualPlotProjection {
    pub points: Vec<ResidualPlotPoint>,
    pub total_count: usize,
    pub matched_count: usize,
    pub sampled: bool,
}

pub enum ResultAnalysisProjection {
    ResidualPlot(ResidualPlotProjection),
    AcfPacf(AcfPacfResult),
    SerialTests(SerialTestsOutput),
    Hypothesis(HypothesisTestOutput),
}

#[derive(Debug, Error)]
pub enum ReportQueryError {
    #[error("report value is not representable")]
    UnrepresentableValue,
    #[error(transparent)]
    Session(#[from] SessionCaptureError),
    #[error("result reference is stale")]
    Stale,
    #[error("result is no longer available")]
    Unavailable,
    #[error("result does not support this report operation")]
    WrongKind,
    #[error("report query is invalid")]
    InvalidRequest,
}

impl ApplicationState {
    pub fn query_result_table(
        &self,
        reference: ResultReference,
        part: ResultTablePart,
        offset: usize,
        limit: usize,
    ) -> Result<ResultPageProjection, ReportQueryError> {
        self.with_linear_regression_summary(reference, |result, summary| {
            let allowed = match part {
                ResultTablePart::Coefficients => {
                    summary.options.coefficient_table
                        || summary.options.coefficient_chart
                        || summary.options.equation
                }
                ResultTablePart::Observations => summary.options.observations,
            };
            if !allowed {
                return Err(ReportQueryError::InvalidRequest);
            }
            table_page(result, part, offset, limit)
        })
    }

    pub fn analyze_result(
        &self,
        reference: ResultReference,
        request: ResultAnalysisRequest,
    ) -> Result<ResultAnalysisProjection, ReportQueryError> {
        self.with_linear_regression_summary(reference, |result, summary| match request {
            ResultAnalysisRequest::ResidualPlot {
                max_points,
                x_range,
            } if summary.options.residual_plot => Ok(ResultAnalysisProjection::ResidualPlot(
                residual_plot(result, max_points, x_range)?,
            )),
            ResultAnalysisRequest::AcfPacf => summary
                .acf
                .as_deref()
                .cloned()
                .map(ResultAnalysisProjection::AcfPacf)
                .ok_or(ReportQueryError::InvalidRequest),
            ResultAnalysisRequest::SerialTests => summary
                .serial
                .as_deref()
                .cloned()
                .map(ResultAnalysisProjection::SerialTests)
                .ok_or(ReportQueryError::InvalidRequest),
            ResultAnalysisRequest::Hypothesis => summary
                .hypothesis
                .as_deref()
                .cloned()
                .map(ResultAnalysisProjection::Hypothesis)
                .ok_or(ReportQueryError::InvalidRequest),
            _ => Err(ReportQueryError::InvalidRequest),
        })
    }

    fn with_linear_regression_summary<T>(
        &self,
        reference: ResultReference,
        read: impl FnOnce(&LinearRegressionResult, &LinearSummary) -> Result<T, ReportQueryError>,
    ) -> Result<T, ReportQueryError> {
        let captured = self.capture_session()?;
        if captured.execution_session_id() != reference.execution_session_id {
            return Err(ReportQueryError::Stale);
        }
        let snapshot = captured
            .execution()
            .query_result(reference.result_id)
            .ok_or(ReportQueryError::Unavailable)?;
        let RuntimeValue::LinearRegression(result) = snapshot.value().value() else {
            return Err(ReportQueryError::WrongKind);
        };
        // Only an executed Summary grants report reads; Fit remains a model input.
        let summary = result
            .summary
            .as_deref()
            .ok_or(ReportQueryError::WrongKind)?;
        let outcome = read(result, summary);
        self.revalidate_captured_session(&captured)
            .map_err(|_| ReportQueryError::Stale)?;
        if captured
            .execution()
            .query_result(reference.result_id)
            .is_none()
        {
            return Err(ReportQueryError::Unavailable);
        }
        outcome
    }
}

pub(super) fn report_projection(
    reference: ResultReference,
    result: &LinearRegressionResult,
    options: &LinearSummaryOptions,
) -> LinearRegressionReportProjection {
    LinearRegressionReportProjection {
        summary: options.clone(),
        reference,
        title: result.report.title.clone(),
        endog_name: result.report.endog_name.clone(),
        param_names: result
            .report
            .coefficients
            .iter()
            .map(|coefficient| coefficient.variable.clone())
            .collect(),
        model: result.report.model_basic_info.clone(),
        condition_number: result.report.diagnostic_info.cond_no,
        coefficient_count: result.report.coefficients.len(),
        observation_count: result.residuals.len(),
    }
}

fn table_page(
    result: &LinearRegressionResult,
    part: ResultTablePart,
    offset: usize,
    limit: usize,
) -> Result<ResultPageProjection, ReportQueryError> {
    if limit == 0 || limit > MAX_RESULT_PAGE_ROWS || offset.checked_add(limit).is_none() {
        return Err(ReportQueryError::InvalidRequest);
    }
    let (count, columns): (_, &[(&str, &str)]) = match part {
        ResultTablePart::Coefficients => (
            result.report.coefficients.len(),
            &[
                ("variable", "Utf8"),
                ("coef", "Float64"),
                ("std_err", "Float64"),
                ("t_value", "Float64"),
                ("p_value", "Float64"),
                ("confidence_interval_0.025", "Float64"),
                ("confidence_interval_0.975", "Float64"),
                ("is_significant", "Boolean"),
            ],
        ),
        ResultTablePart::Observations => (
            result.residuals.len(),
            &[
                ("observation", "UInt64"),
                ("fitted", "Float64"),
                ("residual", "Float64"),
            ],
        ),
    };
    let offset = offset.min(count);
    let end = offset.saturating_add(limit).min(count);
    let values = (offset..end)
        .map(|row| {
            Ok(RuntimeValue::List(match part {
                ResultTablePart::Coefficients => {
                    let coefficient = &result.report.coefficients[row];
                    std::sync::Arc::from([
                        RuntimeValue::Scalar(TabularScalar::String(
                            coefficient.variable.clone().into_boxed_str(),
                        )),
                        RuntimeValue::float64(coefficient.coef)
                            .map_err(|_| ReportQueryError::UnrepresentableValue)?,
                        RuntimeValue::float64(coefficient.std_err)
                            .map_err(|_| ReportQueryError::UnrepresentableValue)?,
                        RuntimeValue::float64(coefficient.t_value)
                            .map_err(|_| ReportQueryError::UnrepresentableValue)?,
                        RuntimeValue::float64(coefficient.p_value)
                            .map_err(|_| ReportQueryError::UnrepresentableValue)?,
                        RuntimeValue::float64(coefficient.ci_lower)
                            .map_err(|_| ReportQueryError::UnrepresentableValue)?,
                        RuntimeValue::float64(coefficient.ci_upper)
                            .map_err(|_| ReportQueryError::UnrepresentableValue)?,
                        RuntimeValue::Scalar(TabularScalar::Bool(coefficient.is_significant)),
                    ])
                }
                ResultTablePart::Observations => std::sync::Arc::from([
                    RuntimeValue::Scalar(TabularScalar::Unsigned((row + 1) as u64)),
                    RuntimeValue::float64(result.fitted[row])
                        .map_err(|_| ReportQueryError::UnrepresentableValue)?,
                    RuntimeValue::float64(result.residuals[row])
                        .map_err(|_| ReportQueryError::UnrepresentableValue)?,
                ]),
            }))
        })
        .collect::<Result<_, ReportQueryError>>()?;
    Ok(ResultPageProjection {
        offset,
        requested_limit: limit,
        total_count: Some(count),
        has_more: end < count,
        kind: ResultPageKind::Sequence,
        columns: columns
            .iter()
            .map(|(name, data_type)| RelationColumn {
                name: (*name).into(),
                data_type: (*data_type).into(),
            })
            .collect(),
        values,
    })
}

fn residual_plot(
    result: &LinearRegressionResult,
    max_points: usize,
    x_range: Option<[f64; 2]>,
) -> Result<ResidualPlotProjection, ReportQueryError> {
    if !(2..=4096).contains(&max_points)
        || x_range.is_some_and(|[min, max]| !min.is_finite() || !max.is_finite() || min > max)
    {
        return Err(ReportQueryError::InvalidRequest);
    }
    let matches = |x: f64| x_range.is_none_or(|[min, max]| x >= min && x <= max);
    let matched_count = result.fitted.iter().filter(|x| matches(**x)).count();
    let take = matched_count.min(max_points);
    let mut points = Vec::with_capacity(take);
    if take > 0 {
        let mut matched = 0usize;
        for (row, (&x, &y)) in result.fitted.iter().zip(&result.residuals).enumerate() {
            if !matches(x) {
                continue;
            }
            // Sample across the whole matching population, retaining paired observations.
            let target = points.len() as u128 * matched_count.saturating_sub(1) as u128
                / take.saturating_sub(1).max(1) as u128;
            if points.len() < take && matched as u128 == target {
                points.push(ResidualPlotPoint {
                    observation: row + 1,
                    x,
                    y,
                });
            }
            matched += 1;
        }
    }
    Ok(ResidualPlotProjection {
        points,
        total_count: result.residuals.len(),
        matched_count,
        sampled: take < matched_count,
    })
}

#[cfg(test)]
pub(crate) mod tests;
