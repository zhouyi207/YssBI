use std::sync::Arc;
use std::time::{Duration, Instant};

use thiserror::Error;
use yss_execution::result::{ResultReference, StoredResult};
use yss_execution::value::RuntimeValue;
use yss_relational_contract::RelationColumn;
use yss_sci_contract::regression::report::OlsModelSummary;
use yss_sci_contract::scientific::{
    AcfPacfRequest, AcfPacfResult, BackendExecutionControl, OlsResult, ScientificBackendError,
};

use super::{MAX_RESULT_PAGE_ROWS, ResultPageKind, ResultPageProjection};
use crate::execution::{ApplicationState, SessionCaptureError};
use crate::hypothesis::{HypothesisApplicationError, HypothesisTestInput, HypothesisTestOutput};
use crate::statistics::{SerialTestsApplicationError, SerialTestsRequest, SerialTestsResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultTablePart {
    Coefficients,
    Observations,
}

pub struct OlsReportProjection {
    pub reference: ResultReference,
    pub title: String,
    pub endog_name: String,
    pub model: OlsModelSummary,
    pub condition_number: f64,
    pub coefficient_count: usize,
    pub observation_count: usize,
}

pub enum ResultAnalysisRequest {
    ResidualPlot {
        max_points: usize,
        x_range: Option<[f64; 2]>,
    },
    AcfPacf {
        max_lag: usize,
    },
    SerialTests {
        lags: usize,
        bg_nomiss0: bool,
    },
    Hypothesis {
        hypothesis: String,
    },
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
    SerialTests(SerialTestsResult),
    Hypothesis(HypothesisTestOutput),
}

#[derive(Debug, Error)]
pub enum ReportQueryError {
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
    #[error(transparent)]
    Backend(#[from] ScientificBackendError),
    #[error(transparent)]
    Hypothesis(#[from] HypothesisApplicationError),
    #[error("serial test failed")]
    Serial(SerialTestsApplicationError),
}

impl ApplicationState {
    pub fn query_ols_report(
        &self,
        reference: ResultReference,
    ) -> Result<OlsReportProjection, ReportQueryError> {
        self.with_ols_result(reference, |result| Ok(report_projection(reference, result)))
    }

    pub fn query_result_table(
        &self,
        reference: ResultReference,
        part: ResultTablePart,
        offset: usize,
        limit: usize,
    ) -> Result<ResultPageProjection, ReportQueryError> {
        self.with_ols_result(reference, |result| table_page(result, part, offset, limit))
    }

    pub fn analyze_result(
        &self,
        reference: ResultReference,
        request: ResultAnalysisRequest,
    ) -> Result<ResultAnalysisProjection, ReportQueryError> {
        self.with_ols_result(reference, |result| {
            Ok(match request {
                ResultAnalysisRequest::ResidualPlot {
                    max_points,
                    x_range,
                } => ResultAnalysisProjection::ResidualPlot(residual_plot(
                    result, max_points, x_range,
                )?),
                ResultAnalysisRequest::AcfPacf { max_lag } => {
                    validate_lags(max_lag)?;
                    let control = BackendExecutionControl::from_shared(
                        Arc::new(std::sync::atomic::AtomicBool::new(false)),
                        Instant::now() + Duration::from_secs(60),
                    );
                    let value = self
                        .scientific_backend()
                        .ok_or(ScientificBackendError::Unavailable)?
                        .acf_pacf(
                            AcfPacfRequest {
                                values: result.residuals.clone(),
                                max_lag,
                            },
                            &control,
                        )?;
                    ResultAnalysisProjection::AcfPacf(value)
                }
                ResultAnalysisRequest::SerialTests { lags, bg_nomiss0 } => {
                    validate_lags(lags)?;
                    let value = crate::statistics::compute_serial_tests(SerialTestsRequest {
                        residuals: result.residuals.clone(),
                        exog: Some(
                            (0..result.residuals.len())
                                .map(|row| result.design.iter().map(|column| column[row]).collect())
                                .collect(),
                        ),
                        lags,
                        bg_nomiss0,
                    })
                    .map_err(ReportQueryError::Serial)?;
                    ResultAnalysisProjection::SerialTests(value)
                }
                ResultAnalysisRequest::Hypothesis { hypothesis } => {
                    if hypothesis.trim().is_empty() || hypothesis.len() > 4096 {
                        return Err(ReportQueryError::InvalidRequest);
                    }
                    let value = crate::hypothesis::run_hypothesis_test(HypothesisTestInput {
                        betas: result.coefficients.clone(),
                        cov_beta: result.report.cov_beta.clone(),
                        df_residual: result.report.model_basic_info.df_residual,
                        param_names: result
                            .report
                            .coefficients
                            .iter()
                            .map(|coefficient| coefficient.variable.clone())
                            .collect(),
                        hypothesis,
                    })?;
                    ResultAnalysisProjection::Hypothesis(value)
                }
            })
        })
    }

    fn with_ols_result<T>(
        &self,
        reference: ResultReference,
        read: impl FnOnce(&OlsResult) -> Result<T, ReportQueryError>,
    ) -> Result<T, ReportQueryError> {
        let captured = self.capture_session()?;
        if captured.execution_session_id() != reference.execution_session_id {
            return Err(ReportQueryError::Stale);
        }
        let snapshot = captured
            .execution()
            .query_result(reference.result_id)
            .ok_or(ReportQueryError::Unavailable)?;
        let StoredResult::Runtime(RuntimeValue::Ols(result)) = snapshot.value().value() else {
            return Err(ReportQueryError::WrongKind);
        };
        // The snapshot retains shared native buffers. No store lock is held during analysis.
        let outcome = read(result);
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

fn report_projection(reference: ResultReference, result: &OlsResult) -> OlsReportProjection {
    OlsReportProjection {
        reference,
        title: result.report.title.clone(),
        endog_name: result.report.endog_name.clone(),
        model: result.report.model_basic_info.clone(),
        condition_number: result.report.diagnostic_info.cond_no,
        coefficient_count: result.report.coefficients.len(),
        observation_count: result.residuals.len(),
    }
}

fn validate_lags(lags: usize) -> Result<(), ReportQueryError> {
    if !(1..=40).contains(&lags) {
        return Err(ReportQueryError::InvalidRequest);
    }
    Ok(())
}

fn table_page(
    result: &OlsResult,
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
            RuntimeValue::List(match part {
                ResultTablePart::Coefficients => {
                    let coefficient = &result.report.coefficients[row];
                    Box::new([
                        RuntimeValue::String(coefficient.variable.clone().into_boxed_str()),
                        RuntimeValue::Decimal(coefficient.coef),
                        RuntimeValue::Decimal(coefficient.std_err),
                        RuntimeValue::Decimal(coefficient.t_value),
                        RuntimeValue::Decimal(coefficient.p_value),
                        RuntimeValue::Decimal(coefficient.ci_lower),
                        RuntimeValue::Decimal(coefficient.ci_upper),
                        RuntimeValue::Bool(coefficient.is_significant),
                    ])
                }
                ResultTablePart::Observations => Box::new([
                    RuntimeValue::Unsigned((row + 1) as u64),
                    RuntimeValue::Decimal(result.fitted[row]),
                    RuntimeValue::Decimal(result.residuals[row]),
                ]),
            })
        })
        .collect();
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
    result: &OlsResult,
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
mod tests;
