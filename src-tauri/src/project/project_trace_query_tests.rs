use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, CorrelationContext, ProjectSessionId, RunId,
    SpanId, SpanKind, SpanOutcome, SpanSpec, TraceSink,
};
use crate::node_system::document::{GraphResourcePath as DocumentGraphPath, GraphRevision};
use crate::node_system::registry::RegistryFingerprint;
use std::collections::BTreeMap;
use std::sync::Arc;

fn active_state(label: &str) -> ProjectState {
    let root = std::env::temp_dir().join(format!(
        "yssbi-trace-query-{label}-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    state
}

fn correlation(graph_path: &str, run_id: Option<u64>) -> CorrelationContext {
    let provenance = CompileProvenance {
        project_session_id: ProjectSessionId::new("query-test-session"),
        graph_path: DocumentGraphPath(graph_path.into()),
        basis: CompilationBasis {
            graph_revision: GraphRevision::new(3),
            registry_fingerprint: RegistryFingerprint::from_bytes([5; 32]),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        },
        compile_id: CompileId::new(11),
    };
    match run_id {
        Some(run_id) => CorrelationContext::compile(&provenance).for_run(RunId::new(run_id), None),
        None => CorrelationContext::compile(&provenance),
    }
}

fn record(
    state: &ProjectState,
    graph_path: &str,
    run_id: Option<u64>,
    parent_span_id: Option<SpanId>,
) -> SpanId {
    let sink = Arc::clone(&state.project_store.read().unwrap().trace_sink);
    let run_id = run_id.map(RunId::new);
    let mut guard = sink.start_span(SpanSpec {
        parent_span_id,
        run_id,
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation(graph_path, run_id.map(RunId::get)),
    });
    let span_id = guard.span_id();
    guard.finish(SpanOutcome::Success);
    span_id
}

#[test]
fn project_trace_query_filters_exact_graph_and_run_oldest_first() {
    let state = active_state("exact-filter");
    let instance_id = state.capture_project_session().unwrap().instance_id;
    let sink = Arc::clone(&state.project_store.read().unwrap().trace_sink);
    let run_id = RunId::new(7);
    let mut parent = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/orders.yssbi-event", Some(7)),
    });
    let first = parent.span_id();
    record(&state, "events/orders-archive.yssbi-event", Some(7), None);
    record(&state, "events/orders.yssbi-event", Some(70), None);
    let mut child = sink.start_span(SpanSpec {
        parent_span_id: Some(first),
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/orders.yssbi-event", Some(7)),
    });
    let last = child.span_id();
    child.finish(SpanOutcome::Success);
    parent.finish(SpanOutcome::Success);

    let graph = state
        .list_graph_traces(
            &instance_id,
            &GraphResourcePath::new("events/orders.yssbi-event").unwrap(),
        )
        .unwrap();
    let run = state.get_run_trace(&instance_id, RunId::new(7)).unwrap();

    assert_eq!(graph.len(), 3);
    assert_eq!(run.len(), 3);
    assert_eq!(graph[0].span_id, first);
    assert_eq!(graph[2].span_id, last);
    assert_eq!(run[0].span_id, first);
    assert_eq!(run[2].span_id, last);
    assert_eq!(run[2].parent_span_id, Some(first));
}

#[test]
fn project_trace_query_returns_empty_for_graph_without_retained_spans() {
    let state = active_state("empty-graph");
    let instance_id = state.capture_project_session().unwrap().instance_id;
    let traces = state
        .list_graph_traces(
            &instance_id,
            &GraphResourcePath::new("events/not-loaded.yssbi-event").unwrap(),
        )
        .unwrap();
    assert!(traces.is_empty());
}

#[test]
fn project_trace_query_reports_evicted_run_as_not_found() {
    let state = active_state("evicted-run");
    state.project_store.write().unwrap().trace_sink =
        Arc::new(crate::node_system::analysis::BoundedTraceSink::new(2).unwrap());
    let instance_id = state.capture_project_session().unwrap().instance_id;
    record(&state, "events/main", Some(7), None);
    record(&state, "events/main", Some(8), None);
    record(&state, "events/main", Some(9), None);
    assert_eq!(
        state
            .get_run_trace(&instance_id, RunId::new(7))
            .unwrap_err(),
        TraceQueryError::NotFound
    );
}

#[test]
fn project_trace_query_rejects_stale_project_identity() {
    let state = active_state("stale-project");
    let stale = state.capture_project_session().unwrap().instance_id;
    state.activate_project_fixture("trace-query-replacement".into(), ProjectData::new());
    assert_eq!(
        state
            .list_graph_traces(
                &stale,
                &GraphResourcePath::new("events/main.yssbi-event").unwrap(),
            )
            .unwrap_err(),
        TraceQueryError::ProjectStale
    );
}

#[test]
fn project_trace_query_isolates_replacement_project_sink() {
    let state = active_state("replacement-isolation");
    let stale = state.capture_project_session().unwrap().instance_id;
    record(&state, "events/main.yssbi-event", Some(7), None);
    state.activate_project_fixture("trace-query-fresh".into(), ProjectData::new());
    let fresh = state.capture_project_session().unwrap().instance_id;
    assert!(
        state
            .list_graph_traces(
                &fresh,
                &GraphResourcePath::new("events/main.yssbi-event").unwrap(),
            )
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        state.get_run_trace(&stale, RunId::new(7)).unwrap_err(),
        TraceQueryError::ProjectStale
    );
}

#[test]
fn project_trace_query_revalidates_after_snapshot() {
    let state = active_state("snapshot-revalidation");
    let expected = state.capture_project_session().unwrap().instance_id;
    record(&state, "events/main", Some(7), None);
    let replacement = state.clone();
    state.set_trace_query_after_snapshot_test_hook(Arc::new(move || {
        replacement.activate_project_fixture("trace-query-interleaved".into(), ProjectData::new());
    }));
    assert_eq!(
        state.get_run_trace(&expected, RunId::new(7)).unwrap_err(),
        TraceQueryError::ProjectStale
    );
}
