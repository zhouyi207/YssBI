#[cfg(test)]
use crate::graph::results::report::LinearRegressionReportProjection;
use crate::graph::results::report::{
    ResultAnalysisProjection, ResultAnalysisRequest, ResultTablePart,
};
use serde::{Deserialize, Serialize};
use yss_graph_execution::identity::ExecutionSessionId;
use yss_graph_execution::result::{ResultId, ResultReference};
#[cfg(test)]
use yss_sci_contract::regression::report::LinearModelSummary;

use super::statistics::{
    AcfPacfResponseDto, DurbinWatsonResultDto, HypothesisTestResponseDto, SerialTestWithLagDto,
    SerialTestsResponseDto,
};
use crate::ipc::error::CommandError;

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
    // Struct variants enforce deny_unknown_fields for these parameter-free reads.
    AcfPacf {},
    SerialTests {},
    Hypothesis {},
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
            ResultAnalysisRequestDto::AcfPacf {} => Self::AcfPacf,
            ResultAnalysisRequestDto::SerialTests {} => Self::SerialTests,
            ResultAnalysisRequestDto::Hypothesis {} => Self::Hypothesis,
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

pub fn graph_result_state_to_dto(
    projection: crate::graph::results::GraphResultState,
) -> Result<yss_ipc_contract::execution::GraphResultStateDto, CommandError> {
    use crate::ipc::channel::execution::{output_dto, port_address_dto};
    use yss_graph_execution::result::{ConnectionCacheState, ResultCacheState};
    use yss_ipc_contract::execution::{
        ConnectionCacheStateDto, ConnectionResultStateDto, GraphResultStateDto,
        OutputResultStateDto, ResultCacheStateDto,
    };
    let outputs = projection
        .outputs
        .iter()
        .map(|(output, state)| {
            Ok(OutputResultStateDto {
                output: crate::ipc::channel::execution::output_dto(output).map_err(|_| {
                    CommandError::diagnosed(
                        "result_source_read_failed",
                        "invalid cached output identity",
                    )
                })?,
                state: match state {
                    ResultCacheState::Missing => ResultCacheStateDto::Missing,
                    ResultCacheState::Stale => ResultCacheStateDto::Stale,
                    ResultCacheState::Valid { .. } => ResultCacheStateDto::Valid,
                },
                result_id: match state {
                    ResultCacheState::Valid { result_id } => Some(result_id.get().to_string()),
                    _ => None,
                },
            })
        })
        .collect::<Result<_, CommandError>>()?;
    let connections = projection
        .connections
        .iter()
        .map(|connection| {
            Ok(ConnectionResultStateDto {
                output: output_dto(&connection.output).map_err(|_| {
                    CommandError::diagnosed(
                        "result_source_read_failed",
                        "invalid connection output",
                    )
                })?,
                input: port_address_dto(&connection.input).map_err(|_| {
                    CommandError::diagnosed("result_source_read_failed", "invalid connection input")
                })?,
                state: match connection.state {
                    ConnectionCacheState::New => ConnectionCacheStateDto::New,
                    ConnectionCacheState::Stale => ConnectionCacheStateDto::Stale,
                    ConnectionCacheState::Valid => ConnectionCacheStateDto::Valid,
                },
            })
        })
        .collect::<Result<_, CommandError>>()?;
    Ok(GraphResultStateDto {
        revision: projection.revision.to_string(),
        execution_session_id: projection.execution_session_id.as_uuid().to_string(),
        semantic_input_hash: projection
            .semantic_input_hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        outputs,
        connections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_analysis_requests_only_select_a_computed_result() {
        assert!(matches!(
            serde_json::from_value::<ResultAnalysisRequestDto>(
                serde_json::json!({ "kind": "acfPacf" })
            )
            .unwrap(),
            ResultAnalysisRequestDto::AcfPacf {}
        ));
        assert!(
            serde_json::from_value::<ResultAnalysisRequestDto>(
                serde_json::json!({ "kind": "acfPacf", "maxLag": 3 })
            )
            .is_err()
        );
        assert!(matches!(
            serde_json::from_value::<ResultAnalysisRequestDto>(
                serde_json::json!({ "kind": "residualPlot", "maxPoints": 100, "xRange": [0, 1] })
            )
            .unwrap(),
            ResultAnalysisRequestDto::ResidualPlot {
                max_points: 100,
                ..
            }
        ));
    }

    #[test]
    fn graph_cache_wire_distinguishes_valid_identities_from_stale_and_missing_outputs() {
        use yss_graph_execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
        use yss_graph_execution::result::ResultCacheState;
        use yss_graph_execution::result::{ConnectionCacheState, ConnectionResultState};
        let state = crate::graph::results::GraphResultState {
            revision: 1,
            execution_session_id: ExecutionSessionId::new(uuid::Uuid::from_u128(1)),
            semantic_input_hash: [0xaa; 32],
            outputs: [
                (
                    "result",
                    ResultCacheState::Valid {
                        result_id: ResultId::from_existing(17),
                    },
                ),
                ("stale", ResultCacheState::Stale),
                ("unavailable", ResultCacheState::Missing),
            ]
            .into_iter()
            .map(|(port, state)| {
                (
                    PlanOutputRef::new(
                        PlanGraphId::from_existing("events/contract.yssbi-event".into()),
                        PlanPortAddress::from_existing(
                            format!("00000000-0000-0000-0000-000000000002:{port}").into(),
                        ),
                    ),
                    state,
                )
            })
            .collect(),
            connections: [
                (3, ConnectionCacheState::Valid),
                (4, ConnectionCacheState::Stale),
                (5, ConnectionCacheState::New),
            ]
            .into_iter()
            .map(|(node, state)| ConnectionResultState {
                output: PlanOutputRef::new(
                    PlanGraphId::from_existing("events/contract.yssbi-event".into()),
                    PlanPortAddress::from_existing(
                        "00000000-0000-0000-0000-000000000002:result".into(),
                    ),
                ),
                input: PlanPortAddress::from_existing(
                    format!("{}:input", uuid::Uuid::from_u128(node)).into(),
                ),
                state,
            })
            .collect(),
        };
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../src/tests/fixtures/node-system-contracts/execution-wire.json"
        ))
        .unwrap();
        assert_eq!(
            serde_json::to_value(graph_result_state_to_dto(state).unwrap()).unwrap(),
            fixture["graphResultState"]
        );
    }

    #[test]
    fn ols_overview_wire_matches_the_frontend_contract_and_stays_bounded() {
        let projection = |n| LinearRegressionReportProjection {
            summary: Default::default(),
            reference: ResultReference {
                execution_session_id: ExecutionSessionId::new(uuid::Uuid::from_u128(1)),
                result_id: ResultId::from_existing(17),
            },
            title: "Linear Regression Summary".into(),
            endog_name: "response".into(),
            param_names: vec!["const".into(), "x".into()],
            condition_number: 2.0,
            coefficient_count: 2,
            observation_count: n,
            model: LinearModelSummary {
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
        let small = crate::result_encoding::report_to_json(projection(3));
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../src/tests/fixtures/node-system-contracts/ols-summary-report.json"
        ))
        .unwrap();
        assert_eq!(small, fixture);
        let large = serde_json::to_vec(&crate::result_encoding::report_to_json(projection(53_940)))
            .unwrap();
        assert!(large.len() < 4096);
        assert!(large.len() <= serde_json::to_vec(&small).unwrap().len() + 16);
    }
}
