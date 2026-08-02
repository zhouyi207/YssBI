use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, CorrelationContext, ProjectSessionId, RunId,
    SpanEvent, SpanKind, SpanStatus, TraceSink,
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

fn event(graph_path: &str, run_id: Option<u64>) -> SpanEvent {
    let provenance = CompileProvenance {
        project_session_id: ProjectSessionId::new("query-test-session"),
        graph_path: DocumentGraphPath(graph_path.into()),
        basis: CompilationBasis {
            graph_revision: GraphRevision::new(3),
            registry_fingerprint: RegistryFingerprint::from_bytes([5; 32]),
            resource_versions: BTreeMap::new(),
        },
        compile_id: CompileId::new(11),
    };
    let correlation = match run_id {
        Some(run_id) => CorrelationContext::compile(&provenance).for_run(RunId::new(run_id), None),
        None => CorrelationContext::compile(&provenance),
    };
    SpanEvent::new(SpanKind::Run, SpanStatus::Started, correlation)
}

fn record(state: &ProjectState, event: SpanEvent) {
    let sink = Arc::clone(&state.project_store.read().unwrap().trace_sink);
    sink.record(event);
}

#[test]
fn project_trace_query_filters_exact_graph_and_run_oldest_first() {
    let state = active_state("exact-filter");
    let instance_id = state.capture_project_session().unwrap().instance_id;
    record(&state, event("events/orders.yssbi-event", Some(7)));
    record(&state, event("events/orders-archive.yssbi-event", Some(7)));
    record(&state, event("events/orders.yssbi-event", Some(70)));
    record(&state, event("events/orders.yssbi-event", Some(7)));

    let graph = state
        .list_graph_traces(
            &instance_id,
            &GraphResourcePath::new("events/orders.yssbi-event").unwrap(),
        )
        .unwrap();
    let run = state.get_run_trace(&instance_id, RunId::new(7)).unwrap();

    assert_eq!(
        graph
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![0, 2, 3]
    );
    assert_eq!(
        run.iter().map(|record| record.sequence).collect::<Vec<_>>(),
        vec![0, 1, 3]
    );
}

#[test]
fn project_trace_query_returns_empty_for_graph_without_retained_records() {
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
    record(&state, event("events/main", Some(7)));
    record(&state, event("events/main", Some(8)));
    record(&state, event("events/main", Some(9)));

    let error = state
        .get_run_trace(&instance_id, RunId::new(7))
        .unwrap_err();

    assert_eq!(error, TraceQueryError::NotFound);
}

#[test]
fn project_trace_query_rejects_stale_project_identity() {
    let state = active_state("stale-project");
    let stale = state.capture_project_session().unwrap().instance_id;
    state.activate_project_fixture("trace-query-replacement".into(), ProjectData::new());

    let error = state
        .list_graph_traces(
            &stale,
            &GraphResourcePath::new("events/main.yssbi-event").unwrap(),
        )
        .unwrap_err();

    assert_eq!(error, TraceQueryError::ProjectStale);
}

#[test]
fn project_trace_query_isolates_replacement_project_sink() {
    let state = active_state("replacement-isolation");
    let stale = state.capture_project_session().unwrap().instance_id;
    record(&state, event("events/main.yssbi-event", Some(7)));
    state.activate_project_fixture("trace-query-fresh".into(), ProjectData::new());
    let fresh = state.capture_project_session().unwrap().instance_id;

    assert_eq!(
        state
            .list_graph_traces(
                &fresh,
                &GraphResourcePath::new("events/main.yssbi-event").unwrap(),
            )
            .unwrap(),
        Vec::new()
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
    record(&state, event("events/main", Some(7)));
    let replacement = state.clone();
    state.set_trace_query_after_snapshot_test_hook(Arc::new(move || {
        replacement.activate_project_fixture("trace-query-interleaved".into(), ProjectData::new());
    }));

    let error = state.get_run_trace(&expected, RunId::new(7)).unwrap_err();

    assert_eq!(error, TraceQueryError::ProjectStale);
}
