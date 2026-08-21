use crate::node_system::analysis::{CompilationBasis, CorrelationContext, ResourceVersionSet};
use crate::node_system::document::{GraphResourcePath, GraphRevision, PortAddressDto};
use crate::node_system::plan::{
    ExecutionDemand, GraphOutputRef, MAX_SAFE_PREVIEW_GENERATION, PlannedValueKind,
    ResultPresentation,
};

use crate::node_system::runtime::{
    DataSeriesMetadata, OrdinaryRunErrorCode, PinResultEntry, ResultFailureCause, ResultState,
    ResultUsage, RunErrorCode, RunErrorOutcome, RunEvent, RunEventKind, RunOutputEvent,
    RunOutputMessage, RunOutputStatus, RunOutputStatusEvent, RunOutputStream, RunPhase,
    StoredResult, StoredValueKind,
};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphOutputRefDto {
    graph_path: String,
    port: PortAddressDto,
}

impl From<GraphOutputRef> for GraphOutputRefDto {
    fn from(output: GraphOutputRef) -> Self {
        Self {
            graph_path: output.graph_path.0.into(),
            port: output.port.into(),
        }
    }
}

impl TryFrom<GraphOutputRefDto> for GraphOutputRef {
    type Error = String;

    fn try_from(output: GraphOutputRefDto) -> Result<Self, Self::Error> {
        Ok(Self {
            graph_path: GraphResourcePath(output.graph_path.into()),
            port: output.port.try_into()?,
        })
    }
}

macro_rules! define_execution_demand_dto {
    ($($variant:ident => $wire_type:literal $({ $($field:ident: $field_type:ty),* $(,)? })?),* $(,)?) => {
        #[cfg(test)]
        pub(crate) const EXECUTION_DEMAND_DTO_WIRE_TYPES: [&str;
            [$(stringify!($variant)),*].len()] = [$($wire_type),*];

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(
            tag = "type",
            rename_all = "camelCase",
            rename_all_fields = "camelCase",
            deny_unknown_fields
        )]
        pub enum ExecutionDemandDto {
            $($variant $({ $($field: $field_type),* })?),*
        }
    };
}

define_execution_demand_dto! {
    Default => "default" {},
    Outputs => "outputs" {
        outputs: Box<[GraphOutputRefDto]>,
        include_default_results: bool,
    },
    PinPreview => "pinPreview" {
        output: GraphOutputRefDto,
        generation: u64,
    },
}

impl TryFrom<ExecutionDemandDto> for ExecutionDemand {
    type Error = String;

    fn try_from(demand: ExecutionDemandDto) -> Result<Self, Self::Error> {
        match demand {
            ExecutionDemandDto::Default {} => Ok(Self::Default),
            ExecutionDemandDto::Outputs {
                outputs,
                include_default_results,
            } => Ok(Self::Outputs {
                outputs: outputs
                    .into_vec()
                    .into_iter()
                    .map(GraphOutputRef::try_from)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
                include_default_results,
            }),
            ExecutionDemandDto::PinPreview { output, generation } => {
                if generation > MAX_SAFE_PREVIEW_GENERATION {
                    return Err(
                        "pin preview generation exceeds JavaScript safe integer range".into(),
                    );
                }
                Ok(Self::PinPreview {
                    output: output.try_into()?,
                    generation,
                })
            }
        }
    }
}

impl From<ExecutionDemand> for ExecutionDemandDto {
    fn from(demand: ExecutionDemand) -> Self {
        match demand {
            ExecutionDemand::Default => Self::Default {},
            ExecutionDemand::Outputs {
                outputs,
                include_default_results,
            } => Self::Outputs {
                outputs: outputs
                    .into_vec()
                    .into_iter()
                    .map(GraphOutputRefDto::from)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                include_default_results,
            },
            ExecutionDemand::PinPreview { output, generation } => Self::PinPreview {
                output: output.into(),
                generation,
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompilationBasisDto {
    graph_revision: String,
    registry_fingerprint: String,
    resource_versions: BTreeMap<String, String>,
}

impl From<CompilationBasis<GraphRevision>> for CompilationBasisDto {
    fn from(basis: CompilationBasis<GraphRevision>) -> Self {
        Self {
            graph_revision: basis.graph_revision.get().to_string(),
            registry_fingerprint: basis.registry_fingerprint.to_hex(),
            resource_versions: resource_versions(basis.resource_versions),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunCorrelationDto {
    project_session_id: String,
    graph_path: String,
    graph_revision: String,
    registry_fingerprint: String,
    resource_versions: BTreeMap<String, String>,
    compile_id: String,
    selection_digest: Option<Box<str>>,
    run_id: Option<String>,
    node_id: Option<String>,
    node_type_id: Option<String>,
    parent_call: Option<String>,
}

impl From<CorrelationContext> for RunCorrelationDto {
    fn from(correlation: CorrelationContext) -> Self {
        Self {
            project_session_id: correlation.project_session_id.as_str().to_owned(),
            graph_path: String::from(correlation.graph_path.0),
            graph_revision: correlation.graph_revision.get().to_string(),
            registry_fingerprint: correlation.registry_fingerprint.to_hex(),
            resource_versions: resource_versions(correlation.resource_versions),
            compile_id: correlation.compile_id.get().to_string(),
            selection_digest: correlation.selection_digest,
            run_id: correlation.run_id.map(|id| id.get().to_string()),
            node_id: correlation.node_id.map(|id| id.to_string()),
            node_type_id: correlation.node_type_id.map(|id| id.as_str().to_owned()),
            parent_call: correlation.parent_call.map(|id| id.get().to_string()),
        }
    }
}

#[cfg(test)]
pub(crate) const RUN_EVENT_KIND_DTO_WIRE_TYPES: [&str; 10] = [
    "runStarted",
    "runCompleted",
    "runErrored",
    "runCancelled",
    "operationStarted",
    "operationCompleted",
    "operationErrored",
    "resultGroupChanged",
    "outputResultChanged",
    "openResultWindow",
];

#[derive(Debug)]
pub(crate) enum RunErrorOutcomeDto {
    Ordinary(OrdinaryRunErrorCode),
    DeadlineExceeded(RunPhase),
}

impl Serialize for RunErrorOutcomeDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        match self {
            Self::Ordinary(code) => {
                map.serialize_entry("code", code)?;
                map.serialize_entry("phase", &Option::<RunPhase>::None)?;
            }
            Self::DeadlineExceeded(phase) => {
                map.serialize_entry("code", &RunErrorCode::DeadlineExceeded)?;
                map.serialize_entry("phase", phase)?;
            }
        }
        map.end()
    }
}

impl From<RunErrorOutcome> for RunErrorOutcomeDto {
    fn from(outcome: RunErrorOutcome) -> Self {
        match outcome {
            RunErrorOutcome::Ordinary { code } => Self::Ordinary(code),
            RunErrorOutcome::DeadlineExceeded { phase } => Self::DeadlineExceeded(phase),
        }
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
    OperationStarted {
        operation_index: u32,
        activation_id: String,
        attempt_id: String,
    },
    OperationCompleted {
        operation_index: u32,
        activation_id: String,
        attempt_id: String,
    },
    OperationErrored {
        operation_index: u32,
        activation_id: String,
        attempt_id: String,
        #[serde(flatten)]
        outcome: RunErrorOutcomeDto,
    },
    ResultGroupChanged {
        activation_id: String,
        result_ids: Box<[String]>,
        state: ResultStateKindDto,
    },
    OutputResultChanged {
        output: GraphOutputRefDto,
        generation: Option<u64>,
        result_id: String,
    },
    OpenResultWindow {
        result_id: String,
    },
}

impl From<RunEventKind> for RunEventKindDto {
    fn from(kind: RunEventKind) -> Self {
        match kind {
            RunEventKind::RunStarted => Self::RunStarted,
            RunEventKind::RunCompleted => Self::RunCompleted,
            RunEventKind::RunErrored { outcome } => Self::RunErrored {
                outcome: outcome.into(),
            },
            RunEventKind::RunCancelled => Self::RunCancelled,
            RunEventKind::OperationStarted {
                operation_index,
                activation_id,
                attempt_id,
            } => Self::OperationStarted {
                operation_index,
                activation_id: activation_id.to_string(),
                attempt_id: attempt_id.to_string(),
            },
            RunEventKind::OperationCompleted {
                operation_index,
                activation_id,
                attempt_id,
            } => Self::OperationCompleted {
                operation_index,
                activation_id: activation_id.to_string(),
                attempt_id: attempt_id.to_string(),
            },
            RunEventKind::OperationErrored {
                operation_index,
                activation_id,
                attempt_id,
                outcome,
            } => Self::OperationErrored {
                operation_index,
                activation_id: activation_id.to_string(),
                attempt_id: attempt_id.to_string(),
                outcome: outcome.into(),
            },
            RunEventKind::ResultGroupChanged {
                activation_id,
                result_ids,
                state,
            } => Self::ResultGroupChanged {
                activation_id: activation_id.to_string(),
                result_ids: result_ids
                    .into_vec()
                    .into_iter()
                    .map(|result_id| result_id.get().to_string())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                state: state.into(),
            },
            RunEventKind::OutputResultChanged {
                output,
                generation,
                result_id,
            } => Self::OutputResultChanged {
                output: output.into(),
                generation,
                result_id: result_id.get().to_string(),
            },
            RunEventKind::OpenResultWindow { result_id } => Self::OpenResultWindow {
                result_id: result_id.get().to_string(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEventDto {
    correlation: RunCorrelationDto,
    basis: CompilationBasisDto,
    kind: RunEventKindDto,
}

impl From<RunEvent> for RunEventDto {
    fn from(event: RunEvent) -> Self {
        Self {
            correlation: event.correlation.into(),
            basis: event.basis.into(),
            kind: event.kind.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RunOutputStreamDto {
    Stdout,
    Stderr,
}

impl From<RunOutputStream> for RunOutputStreamDto {
    fn from(stream: RunOutputStream) -> Self {
        match stream {
            RunOutputStream::Stdout => Self::Stdout,
            RunOutputStream::Stderr => Self::Stderr,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOutputEventDto {
    run_id: String,
    sequence: u64,
    stream: RunOutputStreamDto,
    text: Box<str>,
    source_graph_path: String,
    source_node_id: String,
}

impl From<RunOutputEvent> for RunOutputEventDto {
    fn from(event: RunOutputEvent) -> Self {
        Self {
            run_id: event.run_id.get().to_string(),
            sequence: event.sequence,
            stream: event.stream.into(),
            text: event.text,
            source_graph_path: event.source_graph_path.0.into(),
            source_node_id: event.source_node_id.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RunOutputStatusDto {
    Truncated,
    Dropped,
}

impl From<RunOutputStatus> for RunOutputStatusDto {
    fn from(status: RunOutputStatus) -> Self {
        match status {
            RunOutputStatus::Truncated => Self::Truncated,
            RunOutputStatus::Dropped => Self::Dropped,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOutputStatusEventDto {
    run_id: String,
    sequence: u64,
    stream: RunOutputStreamDto,
    status: RunOutputStatusDto,
    source_graph_path: String,
    source_node_id: String,
}

impl From<RunOutputStatusEvent> for RunOutputStatusEventDto {
    fn from(event: RunOutputStatusEvent) -> Self {
        Self {
            run_id: event.run_id.get().to_string(),
            sequence: event.sequence,
            stream: event.stream.into(),
            status: event.status.into(),
            source_graph_path: event.source_graph_path.0.into(),
            source_node_id: event.source_node_id.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ExecutionChannelEventDto {
    RunEvent(RunEventDto),
    RunOutput(RunOutputEventDto),
    RunOutputStatus(RunOutputStatusEventDto),
}

impl From<RunEvent> for ExecutionChannelEventDto {
    fn from(event: RunEvent) -> Self {
        Self::RunEvent(event.into())
    }
}

impl From<RunOutputMessage> for ExecutionChannelEventDto {
    fn from(message: RunOutputMessage) -> Self {
        match message {
            RunOutputMessage::Output(event) => Self::RunOutput(event.into()),
            RunOutputMessage::Status(event) => Self::RunOutputStatus(event.into()),
        }
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

impl From<crate::node_system::runtime::ResultStateKind> for ResultStateKindDto {
    fn from(state: crate::node_system::runtime::ResultStateKind) -> Self {
        match state {
            crate::node_system::runtime::ResultStateKind::Pending => Self::Pending,
            crate::node_system::runtime::ResultStateKind::Ready => Self::Ready,
            crate::node_system::runtime::ResultStateKind::Failed => Self::Failed,
            crate::node_system::runtime::ResultStateKind::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultProgressDto {
    completed: String,
    total: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ResultFailureCauseDto {
    Execution,
    Upstream { upstream_result_id: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultFailureDto {
    code: &'static str,
    cause: ResultFailureCauseDto,
    upstream_result_ids: Box<[String]>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResultStateDto {
    Pending { progress: ResultProgressDto },
    Ready,
    Failed { failure: ResultFailureDto },
    Cancelled,
}

impl ResultStateDto {
    pub const fn kind(&self) -> ResultStateKindDto {
        match self {
            Self::Pending { .. } => ResultStateKindDto::Pending,
            Self::Ready => ResultStateKindDto::Ready,
            Self::Failed { .. } => ResultStateKindDto::Failed,
            Self::Cancelled => ResultStateKindDto::Cancelled,
        }
    }
}

impl From<&ResultState> for ResultStateDto {
    fn from(state: &ResultState) -> Self {
        match state {
            ResultState::Pending(progress) => Self::Pending {
                progress: ResultProgressDto {
                    completed: progress.completed.to_string(),
                    total: progress.total.map(|total| total.to_string()),
                },
            },
            ResultState::Ready(_) => Self::Ready,
            ResultState::Failed(failure) => Self::Failed {
                failure: match failure.cause {
                    ResultFailureCause::Execution => ResultFailureDto {
                        code: "execution_failed",
                        cause: ResultFailureCauseDto::Execution,
                        upstream_result_ids: Box::default(),
                    },
                    ResultFailureCause::Upstream { upstream_result_id } => ResultFailureDto {
                        code: "upstream_failed",
                        cause: ResultFailureCauseDto::Upstream {
                            upstream_result_id: upstream_result_id.get().to_string(),
                        },
                        upstream_result_ids: vec![upstream_result_id.get().to_string()]
                            .into_boxed_slice(),
                    },
                },
            },
            ResultState::Cancelled => Self::Cancelled,
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
    presentation: ResultPresentation,
    value_kind: ResultValueKindDto,
    metadata: Option<DataSeriesMetadata>,
    total_count: Option<usize>,
    title: Box<str>,
}

impl From<&StoredResult> for ResultDescriptorDto {
    fn from(result: &StoredResult) -> Self {
        let (value_kind, metadata, total_count) = match &result.state {
            ResultState::Ready(value) => (
                match value.kind() {
                    StoredValueKind::Scalar => ResultValueKindDto::Scalar,
                    StoredValueKind::Sequence => ResultValueKindDto::Sequence,
                    StoredValueKind::DataSeries => ResultValueKindDto::DataSeries,
                },
                value.data_series_metadata().cloned(),
                Some(value.len()),
            ),
            _ => (
                match result.contract.kind {
                    PlannedValueKind::Scalar => ResultValueKindDto::Scalar,
                    PlannedValueKind::DataSeries => ResultValueKindDto::DataSeries,
                    PlannedValueKind::DataFrame => ResultValueKindDto::Sequence,
                    PlannedValueKind::Opaque => ResultValueKindDto::Unknown,
                },
                None,
                None,
            ),
        };
        Self {
            result_id: result.id.get().to_string(),
            state: ResultStateDto::from(&result.state),
            provenance: ResultProvenanceDto {
                run_id: result.provenance.run_id.get().to_string(),
                activation_id: result.provenance.activation_id.get().to_string(),
                graph_path: result.provenance.graph_path.0.to_string(),
                graph_revision: result.provenance.graph_revision.get().to_string(),
                node_id: result.provenance.node_id.to_string(),
                output: result.provenance.output.clone().map(Into::into),
                created_at_ms: result.provenance.created_at_ms.to_string(),
            },
            presentation: result.presentation,
            value_kind,
            metadata,
            total_count,
            title: result_title(result),
        }
    }
}

fn result_title(result: &StoredResult) -> Box<str> {
    match result.presentation {
        ResultPresentation::Inspector => result
            .provenance
            .output
            .as_ref()
            .map(|output| output.port.to_string().into_boxed_str())
            .unwrap_or_else(|| "Result".into()),
        ResultPresentation::Plot { .. } => "Plot".into(),
        ResultPresentation::Report { report } => report.canonical_title().into(),
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
    metadata: Option<DataSeriesMetadata>,
    values: Box<[serde_json::Value]>,
}

impl ResultPageDto {
    pub fn new(
        result_id: crate::node_system::runtime::ResultId,
        offset: usize,
        requested_limit: usize,
        value_kind: StoredValueKind,
        metadata: Option<DataSeriesMetadata>,
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
            value_kind: match value_kind {
                StoredValueKind::Scalar => ResultValueKindDto::Scalar,
                StoredValueKind::Sequence => ResultValueKindDto::Sequence,
                StoredValueKind::DataSeries => ResultValueKindDto::DataSeries,
            },
            metadata,
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
    pub fn from_entry(entry: PinResultEntry, result: &StoredResult) -> Self {
        Self {
            result_id: entry.result_id.get().to_string(),
            run_id: entry.run_id.get().to_string(),
            activation_id: entry.activation_id.get().to_string(),
            graph_revision: entry.graph_revision.get().to_string(),
            created_at_ms: entry.created_at_ms.to_string(),
            usage: match entry.usage {
                ResultUsage::Produced => ResultUsageDto::Produced,
                ResultUsage::Reused {
                    original_activation_id,
                } => ResultUsageDto::Reused {
                    original_activation_id: original_activation_id.get().to_string(),
                },
            },
            state: ResultStateDto::from(&result.state),
        }
    }

    pub const fn state_kind(&self) -> ResultStateKindDto {
        self.state.kind()
    }
}

fn resource_versions(versions: ResourceVersionSet) -> BTreeMap<String, String> {
    versions
        .into_iter()
        .map(|(key, version)| (key.as_str().to_owned(), version.as_str().to_owned()))
        .collect()
}

#[cfg(test)]
mod execution_demand_tests {
    use super::*;
    use crate::node_system::document::{PortAddress, PortAddressDto, PortRef};
    use crate::node_system::plan::ExecutionDemand;
    use serde_json::{Value, json};

    const NODE_ID: &str = "00000000-0000-0000-0000-000000000001";
    const INSTANCE_ID: &str = "00000000-0000-0000-0000-000000000002";

    fn decode(value: Value) -> ExecutionDemand {
        serde_json::from_value::<ExecutionDemandDto>(value)
            .unwrap()
            .try_into()
            .unwrap()
    }

    #[test]
    fn execution_demand_default_is_strict_and_has_no_compiler_local_identity() {
        assert_eq!(
            decode(json!({ "type": "default" })),
            ExecutionDemand::Default
        );

        for invalid in [
            json!({}),
            json!({ "type": "unknown" }),
            json!({ "type": "default", "outputs": [] }),
            json!({ "type": "default", "valueIndex": 0 }),
            json!({ "type": "default", "operationIndex": 0 }),
        ] {
            assert!(serde_json::from_value::<ExecutionDemandDto>(invalid).is_err());
        }
    }

    #[test]
    fn execution_demand_outputs_round_trip_declared_instance_empty_and_duplicate_order() {
        let declared = json!({
            "graphPath": "events/Main.yssbi-event",
            "port": { "kind": "declared", "nodeId": NODE_ID, "portKey": "result" }
        });
        let instance = json!({
            "graphPath": "events/Main.yssbi-event",
            "port": {
                "kind": "instance",
                "nodeId": NODE_ID,
                "templateKey": "results",
                "instanceId": INSTANCE_ID
            }
        });
        let wire = json!({
            "type": "outputs",
            "outputs": [declared.clone(), instance.clone(), declared],
            "includeDefaultResults": true
        });

        let demand = decode(wire.clone());
        let ExecutionDemand::Outputs {
            outputs,
            include_default_results,
        } = &demand
        else {
            panic!("expected output demand");
        };
        assert!(include_default_results);
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0], outputs[2]);
        assert!(matches!(outputs[0].port.port, PortRef::Declared { .. }));
        assert!(matches!(outputs[1].port.port, PortRef::Instance { .. }));

        let encoded = ExecutionDemandDto::from(demand);
        assert_eq!(serde_json::to_value(encoded).unwrap(), wire);
        assert_eq!(
            decode(json!({
                "type": "outputs",
                "outputs": [],
                "includeDefaultResults": false
            })),
            ExecutionDemand::Outputs {
                outputs: Box::new([]),
                include_default_results: false,
            }
        );
    }

    #[test]
    fn execution_demand_outputs_reject_missing_extra_and_compiler_local_fields() {
        for invalid in [
            json!({ "type": "outputs", "includeDefaultResults": false }),
            json!({ "type": "outputs", "outputs": [] }),
            json!({
                "type": "outputs",
                "outputs": [],
                "includeDefaultResults": false,
                "extra": true
            }),
            json!({
                "type": "outputs",
                "outputs": [{
                    "graphPath": "events/Main.yssbi-event",
                    "port": { "kind": "declared", "nodeId": NODE_ID, "portKey": "result" },
                    "valueIndex": 1
                }],
                "includeDefaultResults": false
            }),
            json!({
                "type": "outputs",
                "outputs": [{
                    "graphPath": "events/Main.yssbi-event",
                    "port": { "kind": "declared", "nodeId": NODE_ID, "portKey": "result" },
                    "operationIndex": 1
                }],
                "includeDefaultResults": false
            }),
        ] {
            assert!(serde_json::from_value::<ExecutionDemandDto>(invalid).is_err());
        }
    }

    #[test]
    fn pin_preview_demand_rejects_generation_above_javascript_safe_integer() {
        let output = json!({
            "graphPath": "events/Main.yssbi-event",
            "port": { "kind": "declared", "nodeId": NODE_ID, "portKey": "result" }
        });
        let maximum = serde_json::from_value::<ExecutionDemandDto>(json!({
            "type": "pinPreview",
            "output": output.clone(),
            "generation": 9_007_199_254_740_991_u64
        }))
        .unwrap();
        assert!(ExecutionDemand::try_from(maximum).is_ok());

        let unsafe_generation = serde_json::from_value::<ExecutionDemandDto>(json!({
            "type": "pinPreview",
            "output": output,
            "generation": 9_007_199_254_740_992_u64
        }))
        .unwrap();
        assert!(ExecutionDemand::try_from(unsafe_generation).is_err());
    }

    #[test]
    fn output_result_changed_serializes_only_stable_output_and_result_id() {
        let output = GraphOutputRefDto {
            graph_path: "events/Main.yssbi-event".into(),
            port: PortAddressDto::from(PortAddress::declared(
                crate::node_system::document::NodeId::from_uuid(
                    uuid::Uuid::parse_str(NODE_ID).unwrap(),
                ),
                crate::node_system::protocol::PortKey::new("result").unwrap(),
            )),
        };
        let wire = serde_json::to_value(RunEventKindDto::OutputResultChanged {
            output,
            generation: None,
            result_id: "42".into(),
        })
        .unwrap();

        assert_eq!(
            wire,
            json!({
                "type": "outputResultChanged",
                "output": {
                    "graphPath": "events/Main.yssbi-event",
                    "port": { "kind": "declared", "nodeId": NODE_ID, "portKey": "result" }
                },
                "generation": null,
                "resultId": "42"
            })
        );
        assert!(wire.get("valueIndex").is_none());
        assert!(wire.get("operationIndex").is_none());
        assert!(wire.get("name").is_none());
    }

    #[test]
    fn result_dto_serializes_identity_state_and_provenance_without_artifacts() {
        use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
        use crate::node_system::plan::{PlannedValueContract, ResultPresentation, ValueRef};
        use crate::node_system::runtime::RunId;
        use crate::node_system::runtime::{
            ActivationId, ResultFailure, ResultId, ResultProvenance, ResultState, StoredResult,
        };
        use std::sync::Arc;

        let result = StoredResult {
            id: ResultId::new(17),
            provenance: ResultProvenance {
                run_id: RunId::new(5),
                activation_id: ActivationId::next().unwrap(),
                graph_path: GraphResourcePath("events/test.yssbi-event".into()),
                graph_revision: GraphRevision::new(3),
                node_id: NodeId::from_uuid(uuid::Uuid::nil()),
                output: None,
                created_at_ms: 123,
            },
            value: ValueRef::new(0),
            presentation: ResultPresentation::Inspector,
            contract: PlannedValueContract::opaque(),
            state: ResultState::Failed(Arc::new(ResultFailure::new("boom"))),
        };

        let json = serde_json::to_value(ResultDescriptorDto::from(&result)).unwrap();
        assert_eq!(json["resultId"], "17");
        assert_eq!(json["state"]["kind"], "failed");
        assert_eq!(json["state"]["failure"]["code"], "execution_failed");
        assert!(json["state"]["failure"].get("message").is_none());
        assert!(!json.to_string().contains("boom"));
        assert!(
            json["provenance"]["activationId"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .is_ok()
        );

        assert!(!json.to_string().contains("spill"));
    }

    #[test]
    fn upstream_failure_uses_stable_code_and_upstream_result_identity() {
        use crate::node_system::runtime::{ResultFailure, ResultId, ResultState};
        use std::sync::Arc;

        let state = ResultStateDto::from(&ResultState::Failed(Arc::new(ResultFailure::upstream(
            ResultId::new(23),
            "upstream failed",
        ))));
        let json = serde_json::to_value(state).unwrap();

        assert_eq!(json["kind"], "failed");
        assert_eq!(json["failure"]["code"], "upstream_failed");
        assert_eq!(json["failure"]["cause"]["kind"], "upstream");
        assert_eq!(json["failure"]["cause"]["upstreamResultId"], "23");
        assert_eq!(json["failure"]["upstreamResultIds"], json!(["23"]));
        assert!(json["failure"].get("message").is_none());
        assert!(!json.to_string().contains("upstream failed"));
        assert!(json.to_string().find("sourceResultId").is_none());
    }

    #[test]
    fn result_group_and_open_window_wire_include_terminal_state_and_result_id() {
        let group = serde_json::to_value(RunEventKindDto::ResultGroupChanged {
            activation_id: "9".into(),
            result_ids: vec!["17".into()].into_boxed_slice(),
            state: ResultStateKindDto::Failed,
        })
        .unwrap();
        assert_eq!(group["type"], "resultGroupChanged");
        assert_eq!(group["state"], "failed");

        // Task 6 defines this wire shape; Task 9 owns actual View Data emission.
        let window = serde_json::to_value(RunEventKindDto::OpenResultWindow {
            result_id: "17".into(),
        })
        .unwrap();
        assert_eq!(
            window,
            json!({ "type": "openResultWindow", "resultId": "17" })
        );
    }
}
