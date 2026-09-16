use crate::graph::results::{ResultPageKind, ResultPageProjection};
use crate::graph::run::RunDemand;
use crate::ipc::channel::execution::{RunEventDtoError, output_dto};
use serde::Serialize;
use yss_graph_document::GraphResourcePath;
use yss_graph_execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
use yss_graph_execution::result::{ResultId, StoredResultSnapshot};
use yss_ipc_contract::execution::{
    ExecutionDemandDto, GraphOutputRefDto, MAX_SAFE_PREVIEW_GENERATION,
};
use yss_ipc_contract::graph::PortAddressDto;
use yss_node_kernel::RuntimeValue;

fn plan_output_ref(value: GraphOutputRefDto) -> Result<PlanOutputRef, ()> {
    let port: yss_graph_document::PortAddress = value.port.try_into().map_err(|_| ())?;
    let graph = GraphResourcePath::new(value.graph_path).map_err(|_| ())?;
    let graph = PlanGraphId::new(graph.as_str().to_owned().into_boxed_str()).map_err(|_| ())?;
    let port = PlanPortAddress::new(port.to_string().into_boxed_str()).map_err(|_| ())?;
    Ok(PlanOutputRef::new(graph, port))
}

pub(crate) fn execution_demand_to_application(demand: ExecutionDemandDto) -> Result<RunDemand, ()> {
    match demand {
        ExecutionDemandDto::Default => Ok(RunDemand::Default),
        ExecutionDemandDto::Outputs {
            outputs,
            include_default_results,
        } => outputs
            .into_vec()
            .into_iter()
            .map(plan_output_ref)
            .collect::<Result<Vec<_>, _>>()
            .map(|outputs| RunDemand::Outputs {
                outputs: outputs.into_boxed_slice(),
                include_default_results,
            }),
        ExecutionDemandDto::PinPreview { output, generation } => {
            if generation > MAX_SAFE_PREVIEW_GENERATION {
                return Err(());
            }
            Ok(RunDemand::PinPreview {
                output: plan_output_ref(output)?,
                generation,
            })
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultProvenanceDto {
    run_id: String,
    graph_path: String,
    node_id: String,
    output: Option<GraphOutputRefDto>,
    created_at_ms: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResultPresentationDto {
    Inspector,
    Plot { chart: ResultPlotKindDto },
    Report { report: ResultReportKindDto },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultPlotKindDto {
    Scatter,
    Line,
    Plot,
    Ecdf,
    Kde,
    Histogram,
    Correlation,
    Correlogram,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultReportKindDto {
    OlsSummary,
    BinarySummary,
    Iv2slsSummary,
    IvLimlSummary,
    PraisSummary,
    VarSummary,
    VarSoc,
    PanelSummary,
    PanelDid,
    DfAdfSummary,
    DfAdfSummaryList,
    VecSummary,
    VecRankSummary,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultValueKindDto {
    Scalar,
    Sequence,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultDescriptorDto {
    execution_session_id: String,
    result_id: String,
    provenance: ResultProvenanceDto,
    presentation: ResultPresentationDto,
    value_kind: ResultValueKindDto,
    metadata: Option<serde_json::Value>,
    total_count: Option<usize>,
    title: Box<str>,
}

impl ResultDescriptorDto {
    pub(crate) fn from_execution(
        result_id: ResultId,
        result: &StoredResultSnapshot,
    ) -> Result<Self, RunEventDtoError> {
        let stored = result.value().value();
        let (value_kind, total_count) = match stored {
            RuntimeValue::List(values) => (ResultValueKindDto::Sequence, Some(values.len())),
            RuntimeValue::Relation(_) | RuntimeValue::Series(_) => {
                (ResultValueKindDto::Sequence, None)
            }
            _ => (ResultValueKindDto::Scalar, Some(1)),
        };
        let output = result.output();
        let provenance = result.provenance();
        let execution_session_id = provenance
            .reference()
            .execution_session_id
            .as_uuid()
            .to_string();
        let output_dto = output_dto(output)?;
        let node_id = match &output_dto.port {
            PortAddressDto::Declared { node_id, .. } | PortAddressDto::Instance { node_id, .. } => {
                node_id.clone()
            }
        };
        let provenance = ResultProvenanceDto {
            run_id: provenance.run_id().get().to_string(),
            graph_path: output.graph().as_str().to_owned(),
            node_id: node_id.into(),
            output: Some(output_dto),
            created_at_ms: provenance.created_at_ms().to_string(),
        };
        Ok(Self {
            execution_session_id,
            result_id: result_id.get().to_string(),
            provenance,
            presentation: result_presentation(result.value().category()),
            value_kind,
            metadata: None,
            total_count,
            title: "Result".into(),
        })
    }
}

fn result_presentation(
    category: yss_graph_execution::plan::ResultCategory,
) -> ResultPresentationDto {
    match category {
        yss_graph_execution::plan::ResultCategory::Value => ResultPresentationDto::Inspector,
        yss_graph_execution::plan::ResultCategory::PlotData(kind) => ResultPresentationDto::Plot {
            chart: match kind {
                yss_graph_execution::plan::PlotDataKind::Scatter => ResultPlotKindDto::Scatter,
                yss_graph_execution::plan::PlotDataKind::Line => ResultPlotKindDto::Line,
                yss_graph_execution::plan::PlotDataKind::Plot => ResultPlotKindDto::Plot,
                yss_graph_execution::plan::PlotDataKind::Ecdf => ResultPlotKindDto::Ecdf,
                yss_graph_execution::plan::PlotDataKind::Kde => ResultPlotKindDto::Kde,
                yss_graph_execution::plan::PlotDataKind::Histogram => ResultPlotKindDto::Histogram,
                yss_graph_execution::plan::PlotDataKind::Correlation => {
                    ResultPlotKindDto::Correlation
                }
                yss_graph_execution::plan::PlotDataKind::Correlogram => {
                    ResultPlotKindDto::Correlogram
                }
            },
        },
        yss_graph_execution::plan::ResultCategory::StatisticalReport(kind) => {
            ResultPresentationDto::Report {
                report: match kind {
                    yss_graph_execution::plan::StatisticalReportKind::OlsSummary => {
                        ResultReportKindDto::OlsSummary
                    }
                    yss_graph_execution::plan::StatisticalReportKind::BinarySummary => {
                        ResultReportKindDto::BinarySummary
                    }
                    yss_graph_execution::plan::StatisticalReportKind::Iv2slsSummary => {
                        ResultReportKindDto::Iv2slsSummary
                    }
                    yss_graph_execution::plan::StatisticalReportKind::IvLimlSummary => {
                        ResultReportKindDto::IvLimlSummary
                    }
                    yss_graph_execution::plan::StatisticalReportKind::PraisSummary => {
                        ResultReportKindDto::PraisSummary
                    }
                    yss_graph_execution::plan::StatisticalReportKind::VarSummary => {
                        ResultReportKindDto::VarSummary
                    }
                    yss_graph_execution::plan::StatisticalReportKind::VarSoc => {
                        ResultReportKindDto::VarSoc
                    }
                    yss_graph_execution::plan::StatisticalReportKind::PanelSummary => {
                        ResultReportKindDto::PanelSummary
                    }
                    yss_graph_execution::plan::StatisticalReportKind::PanelDid => {
                        ResultReportKindDto::PanelDid
                    }
                    yss_graph_execution::plan::StatisticalReportKind::DfAdfSummary => {
                        ResultReportKindDto::DfAdfSummary
                    }
                    yss_graph_execution::plan::StatisticalReportKind::DfAdfSummaryList => {
                        ResultReportKindDto::DfAdfSummaryList
                    }
                    yss_graph_execution::plan::StatisticalReportKind::VecSummary => {
                        ResultReportKindDto::VecSummary
                    }
                    yss_graph_execution::plan::StatisticalReportKind::VecRankSummary => {
                        ResultReportKindDto::VecRankSummary
                    }
                },
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ResultValueDto {
    Value(serde_json::Value),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultPageDto {
    result_id: String,
    offset: usize,
    requested_limit: usize,
    actual_count: usize,
    total_count: Option<usize>,
    has_more: bool,
    next_offset: Option<usize>,
    value_kind: ResultValueKindDto,
    metadata: Option<ResultTableMetadataDto>,
    values: Box<[serde_json::Value]>,
}

#[derive(Debug, Serialize)]
struct ResultTableMetadataDto {
    columns: Box<[ResultColumnDto]>,
}

#[derive(Debug, Serialize)]
struct ResultColumnDto {
    name: Box<str>,
    #[serde(rename = "type")]
    data_type: Box<str>,
}

impl ResultPageDto {
    pub(crate) fn from_application(
        result_id: ResultId,
        page: ResultPageProjection,
    ) -> Result<Self, RunEventDtoError> {
        let values = page
            .values
            .iter()
            .map(runtime_value_to_json)
            .collect::<Result<Box<[_]>, _>>()?;
        let actual_count = values.len();
        let next = page
            .offset
            .checked_add(actual_count)
            .ok_or(RunEventDtoError::InvalidOutput)?;
        let metadata = (!page.columns.is_empty()).then(|| ResultTableMetadataDto {
            columns: page
                .columns
                .into_vec()
                .into_iter()
                .map(|column| ResultColumnDto {
                    name: column.name,
                    data_type: column.data_type,
                })
                .collect(),
        });
        Ok(Self {
            result_id: result_id.get().to_string(),
            offset: page.offset,
            requested_limit: page.requested_limit,
            actual_count,
            total_count: page.total_count,
            has_more: page.has_more,
            next_offset: page.has_more.then_some(next),
            value_kind: match page.kind {
                ResultPageKind::Scalar => ResultValueKindDto::Scalar,
                ResultPageKind::Sequence => ResultValueKindDto::Sequence,
            },
            metadata,
            values,
        })
    }
}

pub(crate) fn runtime_value_to_json(
    value: &RuntimeValue,
) -> Result<serde_json::Value, RunEventDtoError> {
    Ok(match value {
        RuntimeValue::Null => serde_json::Value::Null,
        RuntimeValue::Bool(value) => (*value).into(),
        RuntimeValue::Integer(value) => serde_json::to_value(
            yss_tabular_contract::TabularScalar::Integer(*value).display_value(),
        )
        .map_err(|_| RunEventDtoError::InvalidOutput)?,
        RuntimeValue::Unsigned(value) => serde_json::to_value(
            yss_tabular_contract::TabularScalar::Unsigned(*value).display_value(),
        )
        .map_err(|_| RunEventDtoError::InvalidOutput)?,
        RuntimeValue::Decimal(value) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .ok_or(RunEventDtoError::InvalidOutput)?,
        RuntimeValue::String(value) | RuntimeValue::Resource(value) => value.as_ref().into(),
        RuntimeValue::Relation(_) | RuntimeValue::Series(_) | RuntimeValue::Ols(_) => {
            return Err(RunEventDtoError::InvalidOutput);
        }
        RuntimeValue::List(values) => values
            .iter()
            .map(runtime_value_to_json)
            .collect::<Result<Vec<_>, _>>()?
            .into(),
        RuntimeValue::Record(values) => values
            .iter()
            .map(|(key, value)| Ok((key.to_string(), runtime_value_to_json(value)?)))
            .collect::<Result<std::collections::BTreeMap<_, _>, RunEventDtoError>>()?
            .into_iter()
            .collect::<serde_json::Map<_, _>>()
            .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_page_wire_keeps_unknown_count_and_exact_wide_integer_text() {
        let page = ResultPageDto::from_application(
            ResultId::from_existing(7),
            ResultPageProjection {
                offset: 0,
                requested_limit: 1,
                total_count: None,
                has_more: true,
                kind: ResultPageKind::Sequence,
                columns: Box::new([yss_relational_contract::RelationColumn {
                    name: "id".into(),
                    data_type: "UInt64".into(),
                }]),
                values: Box::new([RuntimeValue::List(Box::new([RuntimeValue::Unsigned(
                    u64::MAX,
                )]))]),
            },
        )
        .unwrap();
        let encoded = serde_json::to_value(page).unwrap();
        assert!(encoded["totalCount"].is_null());
        assert_eq!(encoded["nextOffset"], 1);
        assert_eq!(encoded["hasMore"], true);
        assert_eq!(encoded["values"][0][0], u64::MAX.to_string());
        assert_eq!(encoded["metadata"]["columns"][0]["type"], "UInt64");
    }
}
