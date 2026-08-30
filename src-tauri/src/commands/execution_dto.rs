use crate::application::execution::run_graph::{
    RunApplicationEvent, RunApplicationEventKind, RunDemand,
};
use crate::execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
use crate::execution::result::{PinResultEntry, ResultId, ResultUsage, StoredResult};
use crate::execution::value::RuntimeValue;
use crate::schema::graph_mutation::PortAddressDto;
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use yss_graph_document::GraphResourcePath;

const MAX_SAFE_PREVIEW_GENERATION: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphOutputRefDto {
    pub(crate) graph_path: String,
    pub(crate) port: PortAddressDto,
}

fn plan_output_ref(value: GraphOutputRefDto) -> Result<PlanOutputRef, ()> {
    let port: yss_graph_document::PortAddress = value.port.try_into().map_err(|_| ())?;
    let graph = GraphResourcePath::new(value.graph_path).map_err(|_| ())?;
    let graph = PlanGraphId::new(graph.as_str().to_owned().into_boxed_str()).map_err(|_| ())?;
    let port = PlanPortAddress::new(port.to_string().into_boxed_str()).map_err(|_| ())?;
    Ok(PlanOutputRef::new(graph, port))
}

fn output_dto(value: &PlanOutputRef) -> Result<GraphOutputRefDto, RunEventDtoError> {
    let parts = value.port().as_str().split(':').collect::<Vec<_>>();
    let port = match parts.as_slice() {
        [node_id, port_key] if uuid::Uuid::parse_str(node_id).is_ok() => PortAddressDto::Declared {
            node_id: (*node_id).into(),
            port_key: (*port_key).into(),
        },
        [node_id, template_key, instance_id]
            if uuid::Uuid::parse_str(node_id).is_ok()
                && uuid::Uuid::parse_str(instance_id).is_ok() =>
        {
            PortAddressDto::Instance {
                node_id: (*node_id).into(),
                template_key: (*template_key).into(),
                instance_id: (*instance_id).into(),
            }
        }
        _ => return Err(RunEventDtoError::InvalidOutput),
    };
    Ok(GraphOutputRefDto {
        graph_path: value.graph().as_str().to_owned(),
        port,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExecutionDemandDto {
    Default,
    Outputs {
        outputs: Box<[GraphOutputRefDto]>,
        include_default_results: bool,
    },
    PinPreview {
        output: GraphOutputRefDto,
        generation: u64,
    },
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

#[derive(Debug)]
pub(crate) enum RunErrorOutcomeDto {
    Failed,
}

impl Serialize for RunErrorOutcomeDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        match self {
            Self::Failed => {
                map.serialize_entry("code", "kernelFailed")?;
                map.serialize_entry("phase", &Option::<&str>::None)?;
            }
        }
        map.end()
    }
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum RunEventKindDto {
    RunStarted,
    RunCompleted,
    RunErrored {
        #[serde(flatten)]
        outcome: RunErrorOutcomeDto,
    },
    RunCancelled,
    PinPreviewResultReady {
        output: GraphOutputRefDto,
        generation: u64,
        result_id: String,
    },
    ResultInspectionRequested {
        result_id: String,
        source: ResultInspectionSourceDto,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResultInspectionSourceDto {
    graph_path: String,
    node_id: Option<String>,
    port_address: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunEventDtoError {
    UnsafePreviewGeneration,
    InvalidOutput,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphRunIdentityDto {
    project_session_id: String,
    graph_path: String,
    run_id: String,
}

#[derive(Debug, Serialize)]
pub struct RunEventDto {
    run: GraphRunIdentityDto,
    kind: RunEventKindDto,
}

impl TryFrom<RunApplicationEvent> for RunEventDto {
    type Error = RunEventDtoError;

    fn try_from(event: RunApplicationEvent) -> Result<Self, Self::Error> {
        let identity = event.identity();
        let run = GraphRunIdentityDto {
            project_session_id: identity.project_session_id().as_str().to_owned(),
            graph_path: identity.graph_path().as_str().to_owned(),
            run_id: identity.run_id().get().to_string(),
        };
        let kind = match event.kind() {
            RunApplicationEventKind::RunStarted => RunEventKindDto::RunStarted,
            RunApplicationEventKind::RunCompleted => RunEventKindDto::RunCompleted,
            RunApplicationEventKind::RunCancelled => RunEventKindDto::RunCancelled,
            RunApplicationEventKind::RunErrored { phase: _ } => RunEventKindDto::RunErrored {
                outcome: RunErrorOutcomeDto::Failed,
            },
            RunApplicationEventKind::PinPreviewResultReady {
                output,
                generation,
                result_id,
            } => {
                if *generation > MAX_SAFE_PREVIEW_GENERATION {
                    return Err(RunEventDtoError::UnsafePreviewGeneration);
                }
                RunEventKindDto::PinPreviewResultReady {
                    output: output_dto(output)?,
                    generation: *generation,
                    result_id: result_id.get().to_string(),
                }
            }
            RunApplicationEventKind::ResultInspectionRequested { result_id, source } => {
                RunEventKindDto::ResultInspectionRequested {
                    result_id: result_id.get().to_string(),
                    source: ResultInspectionSourceDto {
                        graph_path: source.graph().as_str().to_owned(),
                        node_id: source.node().map(|node| node.as_str().to_owned()),
                        port_address: source.port().map(|port| port.as_str().to_owned()),
                    },
                }
            }
        };
        Ok(Self { run, kind })
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ExecutionChannelEventDto {
    RunEvent(RunEventDto),
}

impl TryFrom<RunApplicationEvent> for ExecutionChannelEventDto {
    type Error = RunEventDtoError;

    fn try_from(event: RunApplicationEvent) -> Result<Self, Self::Error> {
        Ok(Self::RunEvent(event.try_into()?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultStateKindDto {
    Pending,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResultStateDto {
    Ready,
}

impl ResultStateDto {
    pub const fn kind(&self) -> ResultStateKindDto {
        match self {
            Self::Ready => ResultStateKindDto::Ready,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultProvenanceDto {
    run_id: String,
    activation_id: String,
    graph_path: String,
    graph_revision: String,
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
    DataSeries,
    Unknown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultDescriptorDto {
    result_id: String,
    state: ResultStateDto,
    provenance: ResultProvenanceDto,
    presentation: ResultPresentationDto,
    value_kind: ResultValueKindDto,
    metadata: Option<serde_json::Value>,
    total_count: Option<usize>,
    title: Box<str>,
}

impl ResultDescriptorDto {
    pub(crate) fn from_execution(result_id: ResultId, result: &StoredResult) -> Self {
        let stored = result.value();
        let (value_kind, total_count) = match stored {
            StoredResult::Categorized { value, .. } => {
                return Self::from_execution(result_id, value);
            }
            StoredResult::Runtime(RuntimeValue::List(values)) => {
                (ResultValueKindDto::Sequence, Some(values.len()))
            }
            StoredResult::Empty => (ResultValueKindDto::Unknown, Some(0)),
            _ => (ResultValueKindDto::Scalar, Some(1)),
        };
        Self {
            result_id: result_id.get().to_string(),
            state: ResultStateDto::Ready,
            provenance: ResultProvenanceDto {
                run_id: result_id.get().to_string(),
                activation_id: result_id.get().to_string(),
                graph_path: "events/application.yssbi-event".into(),
                graph_revision: "0".into(),
                node_id: uuid::Uuid::nil().to_string(),
                output: None,
                created_at_ms: "0".into(),
            },
            presentation: result_presentation(result.category()),
            value_kind,
            metadata: None,
            total_count,
            title: "Result".into(),
        }
    }
}

fn result_presentation(category: crate::execution::plan::ResultCategory) -> ResultPresentationDto {
    match category {
        crate::execution::plan::ResultCategory::Value => ResultPresentationDto::Inspector,
        crate::execution::plan::ResultCategory::PlotData(kind) => ResultPresentationDto::Plot {
            chart: match kind {
                crate::execution::plan::PlotDataKind::Scatter => ResultPlotKindDto::Scatter,
                crate::execution::plan::PlotDataKind::Line => ResultPlotKindDto::Line,
                crate::execution::plan::PlotDataKind::Plot => ResultPlotKindDto::Plot,
                crate::execution::plan::PlotDataKind::Ecdf => ResultPlotKindDto::Ecdf,
                crate::execution::plan::PlotDataKind::Kde => ResultPlotKindDto::Kde,
                crate::execution::plan::PlotDataKind::Histogram => ResultPlotKindDto::Histogram,
                crate::execution::plan::PlotDataKind::Correlation => ResultPlotKindDto::Correlation,
                crate::execution::plan::PlotDataKind::Correlogram => ResultPlotKindDto::Correlogram,
            },
        },
        crate::execution::plan::ResultCategory::StatisticalReport(kind) => {
            ResultPresentationDto::Report {
                report: match kind {
                    crate::execution::plan::StatisticalReportKind::OlsSummary => {
                        ResultReportKindDto::OlsSummary
                    }
                    crate::execution::plan::StatisticalReportKind::BinarySummary => {
                        ResultReportKindDto::BinarySummary
                    }
                    crate::execution::plan::StatisticalReportKind::Iv2slsSummary => {
                        ResultReportKindDto::Iv2slsSummary
                    }
                    crate::execution::plan::StatisticalReportKind::IvLimlSummary => {
                        ResultReportKindDto::IvLimlSummary
                    }
                    crate::execution::plan::StatisticalReportKind::PraisSummary => {
                        ResultReportKindDto::PraisSummary
                    }
                    crate::execution::plan::StatisticalReportKind::VarSummary => {
                        ResultReportKindDto::VarSummary
                    }
                    crate::execution::plan::StatisticalReportKind::VarSoc => {
                        ResultReportKindDto::VarSoc
                    }
                    crate::execution::plan::StatisticalReportKind::PanelSummary => {
                        ResultReportKindDto::PanelSummary
                    }
                    crate::execution::plan::StatisticalReportKind::PanelDid => {
                        ResultReportKindDto::PanelDid
                    }
                    crate::execution::plan::StatisticalReportKind::DfAdfSummary => {
                        ResultReportKindDto::DfAdfSummary
                    }
                    crate::execution::plan::StatisticalReportKind::DfAdfSummaryList => {
                        ResultReportKindDto::DfAdfSummaryList
                    }
                    crate::execution::plan::StatisticalReportKind::VecSummary => {
                        ResultReportKindDto::VecSummary
                    }
                    crate::execution::plan::StatisticalReportKind::VecRankSummary => {
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
    Sequence(Box<[serde_json::Value]>),
    DataSeries(Box<[serde_json::Value]>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultPageDto {
    result_id: String,
    offset: usize,
    requested_limit: usize,
    actual_count: usize,
    total_count: usize,
    has_more: bool,
    next_offset: Option<usize>,
    value_kind: ResultValueKindDto,
    metadata: Option<serde_json::Value>,
    values: Box<[serde_json::Value]>,
}

impl ResultPageDto {
    pub(crate) fn from_execution(
        result_id: ResultId,
        offset: usize,
        requested_limit: usize,
        value_kind: ResultValueKindDto,
        total_count: usize,
        values: Box<[serde_json::Value]>,
    ) -> Self {
        let actual_count = values.len();
        let next = offset.saturating_add(actual_count);
        let has_more = next < total_count;
        Self {
            result_id: result_id.get().to_string(),
            offset,
            requested_limit,
            actual_count,
            total_count,
            has_more,
            next_offset: has_more.then_some(next),
            value_kind,
            metadata: None,
            values,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResultUsageDto {
    Produced,
    Reused { original_activation_id: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinResultEntryDto {
    result_id: String,
    run_id: String,
    activation_id: String,
    graph_revision: String,
    created_at_ms: String,
    usage: ResultUsageDto,
    state: ResultStateDto,
}

impl PinResultEntryDto {
    pub(crate) fn from_execution(entry: PinResultEntry, _result: &StoredResult) -> Self {
        Self {
            result_id: entry.result_id().get().to_string(),
            run_id: entry.run_id().get().to_string(),
            activation_id: entry.activation_id().get().to_string(),
            graph_revision: entry.graph_revision().get().to_string(),
            created_at_ms: entry.created_at_ms().to_string(),
            usage: match entry.usage() {
                ResultUsage::Produced => ResultUsageDto::Produced,
                ResultUsage::Reused {
                    original_activation_id,
                } => ResultUsageDto::Reused {
                    original_activation_id: original_activation_id.get().to_string(),
                },
            },
            state: ResultStateDto::Ready,
        }
    }
}
