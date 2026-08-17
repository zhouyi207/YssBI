use super::*;
use crate::node_system::document::GraphRevision;
use crate::node_system::registry::RegistryFingerprint;
use std::collections::BTreeMap;
use std::sync::Arc;

fn correlation(graph_path: &str, run_id: Option<u64>) -> CorrelationContext {
    correlation_with_compile(graph_path, 1, run_id)
}

fn correlation_with_compile(
    graph_path: &str,
    compile_id: u64,
    run_id: Option<u64>,
) -> CorrelationContext {
    let provenance = CompileProvenance {
        project_session_id: ProjectSessionId::new("project-session"),
        graph_path: crate::node_system::document::GraphResourcePath(graph_path.into()),
        basis: CompilationBasis {
            graph_revision: GraphRevision::new(1),
            registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        },
        compile_id: CompileId::new(compile_id),
    };
    match run_id {
        Some(run_id) => {
            CorrelationContext::compile(&provenance).for_run(RunId::try_new(run_id).unwrap(), None)
        }
        None => CorrelationContext::compile(&provenance),
    }
}

fn finish_run(
    sink: &BoundedTraceSink,
    graph_path: &str,
    run_id: u64,
    child_count: usize,
) -> SpanId {
    let run_id = RunId::new(run_id);
    let mut root = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation(graph_path, Some(run_id.get())),
    });
    let root_id = root.span_id();
    for _ in 0..child_count {
        let mut child = sink.start_span(SpanSpec {
            parent_span_id: Some(root_id),
            run_id: Some(run_id),
            operation_id: None,
            activation_id: None,
            attempt_id: None,
            kind: SpanKind::Run,
            correlation: correlation(graph_path, Some(run_id.get())),
        });
        child.finish(SpanOutcome::Success);
    }
    root.finish(SpanOutcome::Success);
    root_id
}

#[test]
fn trace_retention_rejects_zero_limits() {
    assert!(TraceRetentionPolicy::new(0, 1).is_err());
    assert!(TraceRetentionPolicy::new(1, 0).is_err());
    assert!(
        TraceRetentionPolicy::default()
            .with_max_active_spans_per_bundle(0)
            .is_err()
    );
}

#[test]
fn compilation_bundle_commits_only_when_snapshot_root_completes() {
    let clock = Arc::new(FakeTraceClock::new(MonotonicTimestamp::new(1).unwrap()));
    let sink = BoundedTraceSink::with_clock(TraceRetentionPolicy::default(), clock).unwrap();
    let mut root = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: None,
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Snapshot,
        correlation: correlation("events/compiler", None),
    });
    let mut analysis = sink.start_span(SpanSpec {
        parent_span_id: Some(root.span_id()),
        run_id: None,
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Analysis,
        correlation: correlation("events/compiler", None),
    });
    analysis.finish(SpanOutcome::Success);
    assert!(sink.bundles().is_empty());

    root.finish(SpanOutcome::Success);
    let bundles = sink.bundles();
    let TraceBundle::Compilation(bundle) = &bundles[0] else {
        panic!("expected compilation bundle")
    };
    assert_eq!(bundle.spans.len(), 2);
}

#[test]
fn concurrent_compilations_with_the_same_compile_id_do_not_mix_bundles() {
    let sink = BoundedTraceSink::default();
    let mut first_root = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: None,
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Snapshot,
        correlation: correlation_with_compile("events/first", 7, None),
    });
    let mut second_root = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: None,
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Snapshot,
        correlation: correlation_with_compile("events/second", 7, None),
    });
    let first_root_id = first_root.span_id();
    let second_root_id = second_root.span_id();
    let mut first_analysis = sink.start_span(SpanSpec {
        parent_span_id: Some(first_root_id),
        run_id: None,
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Analysis,
        correlation: correlation_with_compile("events/first", 7, None),
    });
    let mut second_analysis = sink.start_span(SpanSpec {
        parent_span_id: Some(second_root_id),
        run_id: None,
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Analysis,
        correlation: correlation_with_compile("events/second", 7, None),
    });

    first_analysis.finish(SpanOutcome::Success);
    second_analysis.finish(SpanOutcome::Success);
    first_root.finish(SpanOutcome::Success);
    second_root.finish(SpanOutcome::Success);

    let bundles = sink.bundles();
    let compilations = bundles
        .iter()
        .filter_map(|bundle| match bundle {
            TraceBundle::Compilation(bundle) => Some(bundle),
            TraceBundle::Run(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(compilations.len(), 2);
    for (graph_path, root_id) in [
        ("events/first", first_root_id),
        ("events/second", second_root_id),
    ] {
        let bundle = compilations
            .iter()
            .find(|bundle| bundle.graph_path.0.as_ref() == graph_path)
            .expect("each snapshot root commits its own bundle");
        assert_eq!(bundle.compile_id, CompileId::new(7));
        assert_eq!(bundle.spans.len(), 2);
        assert!(bundle.spans.iter().any(|span| span.span_id == root_id));
        assert!(bundle.spans.iter().all(|span| {
            span.correlation.graph_path.0.as_ref() == graph_path
                && span.parent_span_id.is_none_or(|parent| parent == root_id)
        }));
    }
}

#[test]
fn active_run_survives_completed_bundle_eviction_and_commits_atomically() {
    let clock = Arc::new(FakeTraceClock::new(MonotonicTimestamp::new(1).unwrap()));
    let policy = TraceRetentionPolicy::new(1, 64 * 1024).unwrap();
    let sink = BoundedTraceSink::with_clock(policy, clock).unwrap();
    let run_id = RunId::new(1);
    let mut active_root = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/active", Some(1)),
    });
    let root_id = active_root.span_id();
    let mut child = sink.start_span(SpanSpec {
        parent_span_id: Some(root_id),
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/active", Some(1)),
    });
    child.finish(SpanOutcome::Success);

    finish_run(&sink, "events/second", 2, 0);
    finish_run(&sink, "events/third", 3, 0);
    assert!(sink.run_bundle(run_id).is_none());

    active_root.finish(SpanOutcome::Success);
    let bundle = sink
        .run_bundle(run_id)
        .expect("active run commits at root completion");
    assert_eq!(bundle.spans.len(), 2);
    assert!(bundle.spans.iter().all(|span| {
        span.parent_span_id.is_none_or(|parent| {
            bundle
                .spans
                .iter()
                .any(|candidate| candidate.span_id == parent)
        })
    }));
    assert!(sink.run_bundle(RunId::new(3)).is_none());
}

#[test]
fn active_collection_enforces_span_and_byte_budgets_and_commits_drops() {
    let span_policy = TraceRetentionPolicy::new(4, 64 * 1024)
        .unwrap()
        .with_max_active_spans_per_bundle(4)
        .unwrap();
    let span_sink = BoundedTraceSink::new(span_policy).unwrap();
    let run_id = RunId::new(40);
    let mut root = span_sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/span-budget", Some(run_id.get())),
    });
    let root_id = root.span_id();
    for _ in 0..16 {
        let mut child = span_sink.start_span(SpanSpec {
            parent_span_id: Some(root_id),
            run_id: Some(run_id),
            operation_id: None,
            activation_id: None,
            attempt_id: None,
            kind: SpanKind::Run,
            correlation: correlation("events/span-budget", Some(run_id.get())),
        });
        child.finish(SpanOutcome::Success);
    }
    let (active_span_count, _, dropped_before_commit) = span_sink
        .active_run_stats(run_id)
        .expect("run remains active until its root completes");
    assert_eq!(active_span_count, 4);
    assert!(dropped_before_commit > 0);
    root.finish(SpanOutcome::Success);
    let span_bundle = span_sink.run_bundle(run_id).unwrap();
    assert!(span_bundle.metadata.truncated);
    assert!(span_bundle.metadata.dropped_span_count >= dropped_before_commit);

    let byte_limit = 1_024;
    let byte_policy = TraceRetentionPolicy::new(4, byte_limit)
        .unwrap()
        .with_max_active_spans_per_bundle(64)
        .unwrap();
    let byte_sink = BoundedTraceSink::new(byte_policy).unwrap();
    let byte_run_id = RunId::new(41);
    let mut byte_root = byte_sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: Some(byte_run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/byte-budget", Some(byte_run_id.get())),
    });
    let byte_root_id = byte_root.span_id();
    for _ in 0..16 {
        let mut child = byte_sink.start_span(SpanSpec {
            parent_span_id: Some(byte_root_id),
            run_id: Some(byte_run_id),
            operation_id: None,
            activation_id: None,
            attempt_id: None,
            kind: SpanKind::Run,
            correlation: correlation("events/byte-budget", Some(byte_run_id.get())),
        });
        child.finish(SpanOutcome::Success);
    }
    let (_, active_bytes, byte_drops_before_commit) = byte_sink
        .active_run_stats(byte_run_id)
        .expect("run remains active until its root completes");
    assert!(active_bytes <= byte_limit as u64);
    assert!(byte_drops_before_commit > 0);
    byte_root.finish(SpanOutcome::Success);
    let byte_bundle = byte_sink.run_bundle(byte_run_id).unwrap();
    assert!(byte_bundle.metadata.truncated);
    assert!(byte_bundle.metadata.dropped_span_count >= byte_drops_before_commit);
}

#[test]
fn late_spans_do_not_reopen_completed_active_entries() {
    let sink = BoundedTraceSink::default();
    let run_id = RunId::new(50);
    let mut run_root = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/late-run", Some(run_id.get())),
    });
    let mut late_run_child = sink.start_span(SpanSpec {
        parent_span_id: Some(run_root.span_id()),
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/late-run", Some(run_id.get())),
    });
    run_root.finish(SpanOutcome::Success);
    late_run_child.finish(SpanOutcome::Success);

    let mut compilation_root = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: None,
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Snapshot,
        correlation: correlation("events/late-compilation", None),
    });
    let mut late_analysis = sink.start_span(SpanSpec {
        parent_span_id: Some(compilation_root.span_id()),
        run_id: None,
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Analysis,
        correlation: correlation("events/late-compilation", None),
    });
    compilation_root.finish(SpanOutcome::Success);
    late_analysis.finish(SpanOutcome::Success);

    assert_eq!(sink.active_bundle_counts(), (0, 0));
    assert_eq!(sink.run_bundle(run_id).unwrap().spans.len(), 1);
    assert_eq!(sink.bundles().len(), 2);
}

#[test]
fn completed_run_limit_evicts_whole_oldest_run() {
    let sink = BoundedTraceSink::new(TraceRetentionPolicy::new(2, 64 * 1024).unwrap()).unwrap();
    finish_run(&sink, "events/main", 1, 2);
    finish_run(&sink, "events/main", 2, 1);
    finish_run(&sink, "events/main", 3, 0);

    assert!(sink.run_bundle(RunId::new(1)).is_none());
    assert_eq!(sink.run_bundle(RunId::new(2)).unwrap().spans.len(), 2);
    assert_eq!(sink.run_bundle(RunId::new(3)).unwrap().spans.len(), 1);
}

#[test]
fn total_byte_limit_evicts_the_oldest_bundle_atomically() {
    let probe = BoundedTraceSink::default();
    finish_run(&probe, "events/probe", 20, 0);
    let one_bundle_bytes = probe
        .run_bundle(RunId::new(20))
        .unwrap()
        .metadata
        .estimated_bytes as usize;
    let sink = BoundedTraceSink::new(
        TraceRetentionPolicy::new(4, one_bundle_bytes.saturating_add(1)).unwrap(),
    )
    .unwrap();

    finish_run(&sink, "events/first", 21, 0);
    finish_run(&sink, "events/second", 22, 0);

    assert!(sink.run_bundle(RunId::new(21)).is_none());
    assert_eq!(sink.run_bundle(RunId::new(22)).unwrap().spans.len(), 1);
}

#[test]
fn oversized_run_is_explicitly_truncated_without_orphans() {
    let policy = TraceRetentionPolicy::new(4, 1_024).unwrap();
    let sink = BoundedTraceSink::new(policy).unwrap();
    finish_run(&sink, "events/oversized", 9, 24);

    let bundle = sink
        .run_bundle(RunId::new(9))
        .expect("oversized run remains queryable");
    assert!(bundle.metadata.truncated);
    assert!(bundle.metadata.dropped_span_count > 0);
    assert!(bundle.spans.iter().all(|span| {
        span.parent_span_id.is_none_or(|parent| {
            bundle
                .spans
                .iter()
                .any(|candidate| candidate.span_id == parent)
        })
    }));
}

#[test]
fn root_only_soft_byte_floor_is_not_reported_as_truncation() {
    let byte_limit = 1;
    let sink = BoundedTraceSink::new(TraceRetentionPolicy::new(4, byte_limit).unwrap()).unwrap();
    finish_run(&sink, "events/root-floor", 60, 0);

    let bundle = sink.run_bundle(RunId::new(60)).unwrap();
    assert_eq!(bundle.spans.len(), 1);
    assert!(!bundle.metadata.truncated);
    assert_eq!(bundle.metadata.dropped_span_count, 0);
    assert!(bundle.metadata.estimated_bytes > byte_limit as u64);
}

#[test]
fn invalid_completed_bundle_is_rejected_whole_instead_of_repaired_on_query() {
    let sink = BoundedTraceSink::default();
    let run_id = RunId::new(12);
    let mut root = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/invalid", Some(12)),
    });
    let mut orphan = sink.start_span(SpanSpec {
        parent_span_id: Some(SpanId::new(999).unwrap()),
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/invalid", Some(12)),
    });
    orphan.finish(SpanOutcome::Success);
    root.finish(SpanOutcome::Success);

    assert!(sink.run_bundle(run_id).is_none());
    assert!(sink.bundles().is_empty());
}

#[test]
fn graph_query_returns_whole_bundle_for_any_provenance_scope() {
    let sink = BoundedTraceSink::default();
    let run_id = RunId::new(15);
    let mut root = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: correlation("events/main", Some(15)),
    });
    let mut nested_correlation = correlation("functions/shared", Some(15));
    nested_correlation.parent_call = Some(ParentCallId::new(1));
    let mut nested = sink.start_span(SpanSpec {
        parent_span_id: None,
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        correlation: nested_correlation,
    });
    nested.finish(SpanOutcome::Success);
    root.finish(SpanOutcome::Success);

    let bundles = sink.bundles_for_graph(&crate::node_system::document::GraphResourcePath(
        "functions/shared".into(),
    ));
    let TraceBundle::Run(bundle) = &bundles[0] else {
        panic!("expected run bundle")
    };
    assert_eq!(bundle.spans.len(), 2);
    assert_eq!(bundle.metadata.provenance_scopes.len(), 2);
}
