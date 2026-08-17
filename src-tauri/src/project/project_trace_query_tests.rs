use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, CorrelationContext, ProjectSessionId, RunId,
    SpanId, SpanKind, SpanOutcome, SpanSpec, TraceBundle, TraceRetentionPolicy, TraceSink,
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

fn correlation(graph_path: &str, run_id: u64) -> CorrelationContext {
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
    CorrelationContext::compile(&provenance).for_run(RunId::new(run_id), None)
}

fn record_run(
    state: &ProjectState,
    graph_path: &str,
    run_id: u64,
    child_count: usize,
) -> (SpanId, Option<SpanId>) {
    let sink = Arc::clone(&state.project_store.read().unwrap().trace_sink);
    let run_id = RunId::new(run_id);
    let mut root = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation(graph_path, run_id.get()),
    });
    let root_id = root.span_id();
    let mut last = None;
    for _ in 0..child_count {
        let mut child = sink.start_span(SpanSpec {
            parent_span_id: Some(root_id),
            run_id: Some(run_id),
            operation_id: None,
            activation_id: None,
            attempt_id: None,
            kind: SpanKind::Run,
            correlation: correlation(graph_path, run_id.get()),
        });
        last = Some(child.span_id());
        child.finish(SpanOutcome::Success);
    }
    root.finish(SpanOutcome::Success);
    (root_id, last)
}

#[test]
fn project_trace_query_filters_complete_bundles_oldest_first() {
    let state = active_state("exact-filter");
    let instance_id = state.capture_project_session().unwrap().instance_id;
    record_run(&state, "events/orders-archive.yssbi-event", 8, 0);
    record_run(&state, "events/orders.yssbi-event", 70, 0);
    let (first, last) = record_run(&state, "events/orders.yssbi-event", 7, 1);

    let graph = state
        .list_graph_trace_bundles(
            &instance_id,
            &GraphResourcePath::new("events/orders.yssbi-event").unwrap(),
        )
        .unwrap();
    let run = state
        .get_run_trace_bundle(&instance_id, RunId::new(7))
        .unwrap();

    assert_eq!(graph.len(), 2);
    assert!(matches!(&graph[0], TraceBundle::Run(bundle) if bundle.run_id == RunId::new(70)));
    assert!(matches!(&graph[1], TraceBundle::Run(bundle) if bundle.run_id == RunId::new(7)));
    assert_eq!(run.spans.len(), 2);
    assert_eq!(run.spans[0].span_id, first);
    assert_eq!(run.spans[1].span_id, last.unwrap());
    assert_eq!(run.spans[1].parent_span_id, Some(first));
}

#[test]
fn project_trace_query_returns_empty_for_graph_without_retained_bundles() {
    let state = active_state("empty-graph");
    let instance_id = state.capture_project_session().unwrap().instance_id;
    let traces = state
        .list_graph_trace_bundles(
            &instance_id,
            &GraphResourcePath::new("events/not-loaded.yssbi-event").unwrap(),
        )
        .unwrap();
    assert!(traces.is_empty());
}

#[test]
fn project_trace_query_reports_whole_evicted_run_as_not_found() {
    let state = active_state("evicted-run");
    state.project_store.write().unwrap().trace_sink = Arc::new(
        crate::node_system::analysis::BoundedTraceSink::new(
            TraceRetentionPolicy::new(2, 64 * 1024).unwrap(),
        )
        .unwrap(),
    );
    let instance_id = state.capture_project_session().unwrap().instance_id;
    record_run(&state, "events/main", 7, 2);
    record_run(&state, "events/main", 8, 1);
    record_run(&state, "events/main", 9, 0);
    assert_eq!(
        state
            .get_run_trace_bundle(&instance_id, RunId::new(7))
            .unwrap_err(),
        TraceQueryError::NotFound
    );
}

#[test]
fn project_trace_query_copies_incident_association_into_trace_storage() {
    let state = active_state("incident-association");
    let instance_id = state.capture_project_session().unwrap().instance_id;
    record_run(&state, "events/main", 7, 0);

    assert!(
        state
            .associate_run_trace_incident(&instance_id, RunId::new(7), "incident-public-id")
            .unwrap()
    );
    assert_eq!(
        state
            .get_run_trace_bundle(&instance_id, RunId::new(7))
            .unwrap()
            .incident_id
            .as_deref(),
        Some("incident-public-id")
    );
}

#[test]
fn project_trace_query_rejects_stale_project_identity() {
    let state = active_state("stale-project");
    let stale = state.capture_project_session().unwrap().instance_id;
    state.activate_project_fixture("trace-query-replacement".into(), ProjectData::new());
    assert_eq!(
        state
            .list_graph_trace_bundles(
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
    record_run(&state, "events/main.yssbi-event", 7, 0);
    state.activate_project_fixture("trace-query-fresh".into(), ProjectData::new());
    let fresh = state.capture_project_session().unwrap().instance_id;
    assert!(
        state
            .list_graph_trace_bundles(
                &fresh,
                &GraphResourcePath::new("events/main.yssbi-event").unwrap(),
            )
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        state
            .get_run_trace_bundle(&stale, RunId::new(7))
            .unwrap_err(),
        TraceQueryError::ProjectStale
    );
}

#[test]
fn project_trace_query_revalidates_after_bundle_snapshot() {
    let state = active_state("snapshot-revalidation");
    let expected = state.capture_project_session().unwrap().instance_id;
    record_run(&state, "events/main", 7, 0);
    let replacement = state.clone();
    state.set_trace_query_after_snapshot_test_hook(Arc::new(move || {
        replacement.activate_project_fixture("trace-query-interleaved".into(), ProjectData::new());
    }));
    assert_eq!(
        state
            .get_run_trace_bundle(&expected, RunId::new(7))
            .unwrap_err(),
        TraceQueryError::ProjectStale
    );
}
