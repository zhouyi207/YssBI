use crate::error::AppError;
use crate::node_system::analysis::{CorrelationContext, RunId, SpanKind, SpanOutcome, TraceSpan};
use crate::project::{GraphResourcePath, ProjectInstanceId, ProjectState, TraceQueryError};
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceSpanDto {
    span_id: String,
    parent_span_id: Option<String>,
    run_id: Option<String>,
    operation_id: Option<String>,
    activation_id: Option<String>,
    attempt_id: Option<String>,
    kind: TraceKindDto,
    started_at: String,
    finished_at: String,
    outcome: TraceOutcomeDto,
    correlation: TraceCorrelationDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum TraceKindDto {
    Snapshot,
    Analysis,
    Lowering,
    Run,
    OperationAttempt,
    ResourceAcquire,
    AdapterIo,
    ResultPublication,
    Cleanup,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum TraceOutcomeDto {
    Success,
    Error,
    Cancellation,
    Timeout,
    Retry,
    NotReached,
    Cleanup {
        #[serde(rename = "errorCount")]
        error_count: String,
        panicking: bool,
    },
    InternalAborted,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

impl From<TraceSpan> for TraceSpanDto {
    fn from(span: TraceSpan) -> Self {
        Self {
            span_id: span.span_id.get().to_string(),
            parent_span_id: span.parent_span_id.map(|id| id.get().to_string()),
            run_id: span.run_id.map(|id| id.get().to_string()),
            operation_id: span.operation_id.map(|id| id.as_str().to_owned()),
            activation_id: span.activation_id.map(|id| id.get().to_string()),
            attempt_id: span.attempt_id.map(|id| id.get().to_string()),
            kind: span.kind.into(),
            started_at: span.started_at.get().to_string(),
            finished_at: span.finished_at.get().to_string(),
            outcome: span.outcome.into(),
            correlation: span.correlation.into(),
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
            SpanKind::OperationAttempt => Self::OperationAttempt,
            SpanKind::ResourceAcquire => Self::ResourceAcquire,
            SpanKind::AdapterIo => Self::AdapterIo,
            SpanKind::ResultPublication => Self::ResultPublication,
            SpanKind::Cleanup => Self::Cleanup,
        }
    }
}

impl From<SpanOutcome> for TraceOutcomeDto {
    fn from(outcome: SpanOutcome) -> Self {
        match outcome {
            SpanOutcome::Success => Self::Success,
            SpanOutcome::Error => Self::Error,
            SpanOutcome::Cancellation => Self::Cancellation,
            SpanOutcome::Timeout => Self::Timeout,
            SpanOutcome::Retry => Self::Retry,
            SpanOutcome::NotReached => Self::NotReached,
            SpanOutcome::Cleanup {
                error_count,
                panicking,
            } => Self::Cleanup {
                error_count: error_count.to_string(),
                panicking,
            },
            SpanOutcome::InternalAborted => Self::InternalAborted,
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
    let canonical_decimal = !run_id.is_empty()
        && run_id.bytes().all(|byte| byte.is_ascii_digit())
        && (run_id == "0" || !run_id.starts_with('0'));
    canonical_decimal
        .then(|| run_id.parse::<u64>().ok())
        .flatten()
        .and_then(|id| RunId::try_new(id).ok())
        .ok_or_else(|| {
            AppError::new(
                "invalid_opaque_id",
                "runId must be a non-zero unsigned decimal string.",
            )
        })
}

pub(crate) fn list_graph_traces_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
) -> Result<Vec<TraceSpanDto>, AppError> {
    let graph_path = GraphResourcePath::new(graph_path)
        .map_err(|_| AppError::new("invalid_graph_path", "graphPath is invalid."))?;
    state
        .list_graph_traces(&project_instance_id, &graph_path)
        .map(|spans| spans.into_iter().map(TraceSpanDto::from).collect())
        .map_err(trace_error)
}

pub(crate) fn get_run_trace_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    run_id: String,
) -> Result<Vec<TraceSpanDto>, AppError> {
    state
        .get_run_trace(&project_instance_id, parse_run_id(&run_id)?)
        .map(|spans| spans.into_iter().map(TraceSpanDto::from).collect())
        .map_err(trace_error)
}

#[tauri::command]
pub fn list_graph_traces(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
) -> Result<Vec<TraceSpanDto>, AppError> {
    list_graph_traces_from_state(state.inner(), project_instance_id, graph_path)
}

#[tauri::command]
pub fn get_run_trace(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    run_id: String,
) -> Result<Vec<TraceSpanDto>, AppError> {
    get_run_trace_from_state(state.inner(), project_instance_id, run_id)
}
