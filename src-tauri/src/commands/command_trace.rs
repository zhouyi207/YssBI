use crate::error::CommandError;
use crate::node_system::analysis::{
    CompilationTraceBundle, CorrelationContext, RunTraceBundle, SpanKind, SpanOutcome, TraceBundle,
    TraceBundleMetadata, TraceProvenanceScope, TraceSpan,
};
use crate::node_system::runtime::RunId;
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceProvenanceScopeDto {
    project_session_id: String,
    graph_path: String,
    graph_revision: String,
    registry_fingerprint: String,
    resource_versions: BTreeMap<String, String>,
    compile_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceBundleMetadataDto {
    provenance_scopes: Vec<TraceProvenanceScopeDto>,
    truncated: bool,
    dropped_span_count: String,
    estimated_bytes: String,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "bundleKind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TraceBundleDto {
    Compilation {
        compile_id: String,
        graph_path: String,
        metadata: TraceBundleMetadataDto,
        spans: Vec<TraceSpanDto>,
    },
    Run {
        run_id: String,
        compile_id: String,
        graph_path: String,
        selection_digest: Option<Box<str>>,
        incident_id: Option<Box<str>>,
        metadata: TraceBundleMetadataDto,
        spans: Vec<TraceSpanDto>,
    },
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

impl From<TraceProvenanceScope> for TraceProvenanceScopeDto {
    fn from(scope: TraceProvenanceScope) -> Self {
        Self {
            project_session_id: scope.project_session_id.as_str().to_owned(),
            graph_path: String::from(scope.graph_path.0),
            graph_revision: scope.graph_revision.get().to_string(),
            registry_fingerprint: scope.registry_fingerprint.to_hex(),
            resource_versions: scope
                .resource_versions
                .into_iter()
                .map(|(key, version)| (key.as_str().to_owned(), version.as_str().to_owned()))
                .collect(),
            compile_id: scope.compile_id.get().to_string(),
        }
    }
}

impl From<TraceBundleMetadata> for TraceBundleMetadataDto {
    fn from(metadata: TraceBundleMetadata) -> Self {
        Self {
            provenance_scopes: metadata
                .provenance_scopes
                .into_vec()
                .into_iter()
                .map(TraceProvenanceScopeDto::from)
                .collect(),
            truncated: metadata.truncated,
            dropped_span_count: metadata.dropped_span_count.to_string(),
            estimated_bytes: metadata.estimated_bytes.to_string(),
        }
    }
}

impl From<CompilationTraceBundle> for TraceBundleDto {
    fn from(bundle: CompilationTraceBundle) -> Self {
        Self::Compilation {
            compile_id: bundle.compile_id.get().to_string(),
            graph_path: String::from(bundle.graph_path.0),
            metadata: bundle.metadata.into(),
            spans: bundle
                .spans
                .into_vec()
                .into_iter()
                .map(TraceSpanDto::from)
                .collect(),
        }
    }
}

impl From<RunTraceBundle> for TraceBundleDto {
    fn from(bundle: RunTraceBundle) -> Self {
        Self::Run {
            run_id: bundle.run_id.get().to_string(),
            compile_id: bundle.compile_id.get().to_string(),
            graph_path: String::from(bundle.graph_path.0),
            selection_digest: bundle.selection_digest,
            incident_id: bundle.incident_id,
            metadata: bundle.metadata.into(),
            spans: bundle
                .spans
                .into_vec()
                .into_iter()
                .map(TraceSpanDto::from)
                .collect(),
        }
    }
}

impl From<TraceBundle> for TraceBundleDto {
    fn from(bundle: TraceBundle) -> Self {
        match bundle {
            TraceBundle::Compilation(bundle) => bundle.into(),
            TraceBundle::Run(bundle) => bundle.into(),
        }
    }
}

fn trace_error(error: TraceQueryError) -> CommandError {
    match error {
        TraceQueryError::ProjectStale => CommandError::expected("trace_project_stale"),
        TraceQueryError::NotFound => CommandError::expected("trace_not_found"),
    }
}

fn parse_run_id(run_id: &str) -> Result<RunId, CommandError> {
    let canonical_decimal = !run_id.is_empty()
        && run_id.bytes().all(|byte| byte.is_ascii_digit())
        && (run_id == "0" || !run_id.starts_with('0'));
    canonical_decimal
        .then(|| run_id.parse::<u64>().ok())
        .flatten()
        .and_then(|id| RunId::try_new(id).ok())
        .ok_or_else(|| CommandError::expected("invalid_opaque_id"))
}

pub(crate) fn list_graph_trace_bundles_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
) -> Result<Vec<TraceBundleDto>, CommandError> {
    let graph_path = GraphResourcePath::new(graph_path)
        .map_err(|_| CommandError::expected("invalid_graph_path"))?;
    state
        .list_graph_trace_bundles(&project_instance_id, &graph_path)
        .map(|bundles| bundles.into_iter().map(TraceBundleDto::from).collect())
        .map_err(trace_error)
}

pub(crate) fn get_run_trace_bundle_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    run_id: String,
) -> Result<TraceBundleDto, CommandError> {
    state
        .get_run_trace_bundle(&project_instance_id, parse_run_id(&run_id)?)
        .map(TraceBundleDto::from)
        .map_err(trace_error)
}

#[tauri::command]
pub fn list_graph_trace_bundles(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
) -> Result<Vec<TraceBundleDto>, CommandError> {
    list_graph_trace_bundles_from_state(state.inner(), project_instance_id, graph_path)
}

#[tauri::command]
pub fn get_run_trace_bundle(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    run_id: String,
) -> Result<TraceBundleDto, CommandError> {
    get_run_trace_bundle_from_state(state.inner(), project_instance_id, run_id)
}
