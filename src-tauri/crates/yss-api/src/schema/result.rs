use serde::{Deserialize, Serialize};
use yss_application::execution::result_query::report::{
    OlsReportProjection, ResultAnalysisProjection, ResultAnalysisRequest, ResultTablePart,
};
use yss_execution::identity::ExecutionSessionId;
use yss_execution::result::{ResultId, ResultReference};
use yss_sci_contract::regression::report::OlsModelSummary;

use super::statistics::{
    AcfPacfResponseDto, DurbinWatsonResultDto, HypothesisTestResponseDto, SerialTestWithLagDto,
    SerialTestsResponseDto,
};
use crate::error::CommandError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultReferenceDto {
    pub execution_session_id: String,
    pub result_id: String,
}

impl TryFrom<ResultReferenceDto> for ResultReference {
    type Error = CommandError;

    fn try_from(value: ResultReferenceDto) -> Result<Self, Self::Error> {
        let invalid = || CommandError::expected("invalid_result_reference");
        let session = uuid::Uuid::parse_str(&value.execution_session_id).map_err(|_| invalid())?;
        let result_id: u64 = value.result_id.parse().map_err(|_| invalid())?;
        if result_id == 0 || result_id.to_string() != value.result_id {
            return Err(invalid());
        }
        Ok(Self {
            execution_session_id: ExecutionSessionId::new(session),
            result_id: ResultId::from_existing(result_id),
        })
    }
}

impl From<ResultReference> for ResultReferenceDto {
    fn from(value: ResultReference) -> Self {
        Self {
            execution_session_id: value.execution_session_id.as_uuid().to_string(),
            result_id: value.result_id.get().to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultTablePartDto {
    Coefficients,
    Observations,
}

impl From<ResultTablePartDto> for ResultTablePart {
    fn from(value: ResultTablePartDto) -> Self {
        match value {
            ResultTablePartDto::Coefficients => Self::Coefficients,
            ResultTablePartDto::Observations => Self::Observations,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultTableReferenceDto {
    kind: &'static str,
    part: ResultTablePartDto,
    row_count: usize,
}

#[derive(Serialize)]
pub struct OlsReportDto {
    title: String,
    endog_name: String,
    #[serde(rename = "resultRef")]
    reference: ResultReferenceDto,
    model_basic_info: OlsModelSummary,
    diagnostic_info: OlsDiagnosticsDto,
    coefficients: ResultTableReferenceDto,
    observations: ResultTableReferenceDto,
}

#[derive(Serialize)]
struct OlsDiagnosticsDto {
    cond_no: f64,
}

impl From<OlsReportProjection> for OlsReportDto {
    fn from(value: OlsReportProjection) -> Self {
        Self {
            title: value.title,
            endog_name: value.endog_name,
            reference: value.reference.into(),
            model_basic_info: value.model,
            diagnostic_info: OlsDiagnosticsDto {
                cond_no: value.condition_number,
            },
            coefficients: ResultTableReferenceDto {
                kind: "tableRef",
                part: ResultTablePartDto::Coefficients,
                row_count: value.coefficient_count,
            },
            observations: ResultTableReferenceDto {
                kind: "tableRef",
                part: ResultTablePartDto::Observations,
                row_count: value.observation_count,
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResultAnalysisRequestDto {
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

impl From<ResultAnalysisRequestDto> for ResultAnalysisRequest {
    fn from(value: ResultAnalysisRequestDto) -> Self {
        match value {
            ResultAnalysisRequestDto::ResidualPlot {
                max_points,
                x_range,
            } => Self::ResidualPlot {
                max_points,
                x_range,
            },
            ResultAnalysisRequestDto::AcfPacf { max_lag } => Self::AcfPacf { max_lag },
            ResultAnalysisRequestDto::SerialTests { lags, bg_nomiss0 } => {
                Self::SerialTests { lags, bg_nomiss0 }
            }
            ResultAnalysisRequestDto::Hypothesis { hypothesis } => Self::Hypothesis { hypothesis },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualPlotDto {
    points: Vec<ResidualPlotPointDto>,
    total_count: usize,
    matched_count: usize,
    sampled: bool,
    sampling: &'static str,
}

#[derive(Serialize)]
pub struct ResidualPlotPointDto {
    observation: usize,
    x: f64,
    y: f64,
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ResultAnalysisResponseDto {
    ResidualPlot(ResidualPlotDto),
    AcfPacf(AcfPacfResponseDto),
    SerialTests(SerialTestsResponseDto),
    Hypothesis(HypothesisTestResponseDto),
}

impl From<ResultAnalysisProjection> for ResultAnalysisResponseDto {
    fn from(value: ResultAnalysisProjection) -> Self {
        match value {
            ResultAnalysisProjection::ResidualPlot(value) => Self::ResidualPlot(ResidualPlotDto {
                points: value
                    .points
                    .into_iter()
                    .map(|point| ResidualPlotPointDto {
                        observation: point.observation,
                        x: point.x,
                        y: point.y,
                    })
                    .collect(),
                total_count: value.total_count,
                matched_count: value.matched_count,
                sampled: value.sampled,
                sampling: "systematic",
            }),
            ResultAnalysisProjection::AcfPacf(value) => Self::AcfPacf(AcfPacfResponseDto {
                acf: value.acf,
                pacf: value.pacf,
                n: value.n,
            }),
            ResultAnalysisProjection::SerialTests(value) => {
                Self::SerialTests(SerialTestsResponseDto {
                    bg: value.bg.map(|v| SerialTestWithLagDto {
                        stat: v.stat,
                        p_value: v.p_value,
                        lags: v.lags,
                    }),
                    q: value.q.map(|v| SerialTestWithLagDto {
                        stat: v.stat,
                        p_value: v.p_value,
                        lags: v.lags,
                    }),
                    dw: DurbinWatsonResultDto { d: value.dw.d },
                })
            }
            ResultAnalysisProjection::Hypothesis(value) => Self::Hypothesis(value.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ols_overview_wire_matches_the_frontend_contract_and_stays_bounded() {
        let projection = |n| OlsReportProjection {
            reference: ResultReference {
                execution_session_id: ExecutionSessionId::new(uuid::Uuid::from_u128(1)),
                result_id: ResultId::from_existing(17),
            },
            title: "OLS Summary".into(),
            endog_name: "response".into(),
            condition_number: 2.0,
            coefficient_count: 2,
            observation_count: n,
            model: OlsModelSummary {
                model_type: "OLS".into(),
                method: "Least Squares".into(),
                num_observation: n,
                r_squared: 0.875,
                adj_r_squared: 0.75,
                f_statistic: 7.0,
                prob_f_statistic: 0.1,
                df_model: 1,
                df_residual: 1,
                df_total: 2,
                ss_model: 1.125,
                ss_residual: 0.125,
                ss_total: 1.25,
                ms_model: 1.125,
                ms_residual: 0.125,
                ms_total: 0.625,
                covariance_type: "nonrobust".into(),
            },
        };
        let small = serde_json::to_value(OlsReportDto::from(projection(3))).unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../src/tests/fixtures/node-system-contracts/ols-summary-report.json"
        ))
        .unwrap();
        assert_eq!(small, fixture);
        let large = serde_json::to_vec(&OlsReportDto::from(projection(53_940))).unwrap();
        assert!(large.len() < 4096);
        assert!(large.len() <= serde_json::to_vec(&small).unwrap().len() + 16);
    }
}
