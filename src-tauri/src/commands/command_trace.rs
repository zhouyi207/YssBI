use crate::error::AppError;
use crate::node_system::analysis::{
    CorrelationContext, RunId, SpanKind, SpanStatus, TraceRecord, TraceValue,
};
use crate::project::{GraphResourcePath, ProjectInstanceId, ProjectState, TraceQueryError};
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceRecordDto {
    sequence: String,
    kind: TraceKindDto,
    status: TraceStatusDto,
    correlation: TraceCorrelationDto,
    fields: BTreeMap<String, TraceValueDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum TraceKindDto {
    Snapshot,
    Analysis,
    Lowering,
    Run,
    Operation,
    RelationalBackend,
    ResourceAcquire,
    Cleanup,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum TraceStatusDto {
    Started,
    Succeeded,
    Failed,
    Cancelled,
    Blocked,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceCorrelationDto {
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

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum TraceValueDto {
    Integer { value: i64 },
    Text { value: String },
    Redacted,
}

impl From<TraceRecord> for TraceRecordDto {
    fn from(record: TraceRecord) -> Self {
        Self {
            sequence: record.sequence.to_string(),
            kind: record.event.kind.into(),
            status: record.event.status.into(),
            correlation: record.event.correlation.into(),
            fields: public_fields(record.event.fields),
        }
    }
}

impl From<SpanKind> for TraceKindDto {
    fn from(kind: SpanKind) -> Self {
        match kind {
            SpanKind::Snapshot => Self::Snapshot,
            SpanKind::Analysis => Self::Analysis,
            SpanKind::Lowering => Self::Lowering,
            SpanKind::Run => Self::Run,
            SpanKind::Operation => Self::Operation,
            SpanKind::RelationalBackend => Self::RelationalBackend,
            SpanKind::ResourceAcquire => Self::ResourceAcquire,
            SpanKind::Cleanup => Self::Cleanup,
        }
    }
}

impl From<SpanStatus> for TraceStatusDto {
    fn from(status: SpanStatus) -> Self {
        match status {
            SpanStatus::Started => Self::Started,
            SpanStatus::Succeeded => Self::Succeeded,
            SpanStatus::Failed => Self::Failed,
            SpanStatus::Cancelled => Self::Cancelled,
            SpanStatus::Blocked => Self::Blocked,
        }
    }
}

impl From<CorrelationContext> for TraceCorrelationDto {
    fn from(correlation: CorrelationContext) -> Self {
        Self {
            project_session_id: correlation.project_session_id.as_str().to_owned(),
            graph_path: String::from(correlation.graph_path.0),
            graph_revision: correlation.graph_revision.get().to_string(),
            registry_fingerprint: correlation.registry_fingerprint.to_hex(),
            resource_versions: correlation
                .resource_versions
                .into_iter()
                .map(|(key, version)| (key.as_str().to_owned(), version.as_str().to_owned()))
                .collect(),
            compile_id: correlation.compile_id.get().to_string(),
            selection_digest: correlation.selection_digest,
            run_id: correlation.run_id.map(|id| id.get().to_string()),
            node_id: correlation.node_id.map(|id| id.to_string()),
            node_type_id: correlation.node_type_id.map(|id| id.as_str().to_owned()),
            parent_call: correlation.parent_call.map(|id| id.get().to_string()),
        }
    }
}

fn public_fields(fields: BTreeMap<Box<str>, TraceValue>) -> BTreeMap<String, TraceValueDto> {
    fields
        .into_iter()
        .filter_map(|(key, value)| {
            allowed_public_field(&key, value).map(|value| (key.into(), value))
        })
        .collect()
}

fn allowed_public_field(key: &str, value: TraceValue) -> Option<TraceValueDto> {
    match (key, value) {
        ("backendId", TraceValue::Text(value)) => Some(TraceValueDto::Text {
            value: value.into(),
        }),
        ("subplanIndex", TraceValue::Integer(value)) => Some(TraceValueDto::Integer { value }),
        ("backendId" | "subplanIndex", TraceValue::Redacted) => Some(TraceValueDto::Redacted),
        _ => None,
    }
}

fn trace_error(error: TraceQueryError) -> AppError {
    match error {
        TraceQueryError::ProjectStale => AppError::new(
            "trace_project_stale",
            "The active project changed; refresh trace details.",
        ),
        TraceQueryError::NotFound => AppError::new(
            "trace_not_found",
            "The requested trace is no longer retained.",
        ),
    }
}

fn parse_run_id(run_id: &str) -> Result<RunId, AppError> {
    run_id.parse::<u64>().map(RunId::new).map_err(|_| {
        AppError::new(
            "invalid_opaque_id",
            "runId must be an unsigned decimal string.",
        )
    })
}

pub(crate) fn list_graph_traces_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
) -> Result<Vec<TraceRecordDto>, AppError> {
    let graph_path = GraphResourcePath::new(graph_path)
        .map_err(|_| AppError::new("invalid_graph_path", "graphPath is invalid."))?;
    state
        .list_graph_traces(&project_instance_id, &graph_path)
        .map(|records| records.into_iter().map(TraceRecordDto::from).collect())
        .map_err(trace_error)
}

pub(crate) fn get_run_trace_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    run_id: String,
) -> Result<Vec<TraceRecordDto>, AppError> {
    state
        .get_run_trace(&project_instance_id, parse_run_id(&run_id)?)
        .map(|records| records.into_iter().map(TraceRecordDto::from).collect())
        .map_err(trace_error)
}

#[tauri::command]
pub fn list_graph_traces(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
) -> Result<Vec<TraceRecordDto>, AppError> {
    list_graph_traces_from_state(state.inner(), project_instance_id, graph_path)
}

#[tauri::command]
pub fn get_run_trace(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    run_id: String,
) -> Result<Vec<TraceRecordDto>, AppError> {
    get_run_trace_from_state(state.inner(), project_instance_id, run_id)
}
