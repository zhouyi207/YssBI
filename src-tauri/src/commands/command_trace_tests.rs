use super::command_trace::{TraceBundleDto, get_run_trace_bundle_from_state};
use crate::node_system::ProjectSessionId;
use crate::node_system::analysis::{
    CompileId, CorrelationContext, MonotonicTimestamp, ParentCallId, ResourceKey, ResourceVersion,
    RunTraceBundle, SpanId, SpanKind, SpanOutcome, TraceBundleMetadata, TraceProvenanceScope,
    TraceSpan,
};
use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
use crate::node_system::registry::RegistryFingerprint;
use crate::node_system::runtime::RunId;
use crate::project::{ProjectData, ProjectState};
use std::collections::BTreeMap;

#[test]
fn command_trace_bundle_dto_uses_exact_decimal_wire_and_metadata() {
    let unsafe_id = 9_007_199_254_740_993_u64;
    let node_id = NodeId::new();
    let run_id = RunId::new(unsafe_id);
    let root_span_id = SpanId::new(unsafe_id + 1).unwrap();
    let correlation = CorrelationContext {
        project_session_id: ProjectSessionId::new("session-7"),
        graph_path: GraphResourcePath("events/main.yssbi-event".into()),
        graph_revision: GraphRevision::new(unsafe_id),
        registry_fingerprint: RegistryFingerprint::from_bytes([2; 32]),
        resource_versions: BTreeMap::from([(
            ResourceKey::new("functions/shared"),
            ResourceVersion::new("9"),
        )]),
        compile_id: CompileId::new(unsafe_id),
        selection_digest: Some("demand-selection-a".into()),
        run_id: Some(run_id),
        node_id: Some(node_id),
        node_type_id: None,
        parent_call: Some(ParentCallId::new(unsafe_id)),
        trace_parent_span_id: Some(root_span_id),
    };
    let root = TraceSpan {
        span_id: root_span_id,
        parent_span_id: None,
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Run,
        started_at: MonotonicTimestamp::new(unsafe_id + 2).unwrap(),
        finished_at: MonotonicTimestamp::new(unsafe_id + 7).unwrap(),
        outcome: SpanOutcome::Success,
        correlation: CorrelationContext {
            parent_call: None,
            trace_parent_span_id: None,
            ..correlation.clone()
        },
    };
    let cleanup = TraceSpan {
        span_id: SpanId::new(unsafe_id + 3).unwrap(),
        parent_span_id: Some(root_span_id),
        run_id: Some(run_id),
        operation_id: None,
        activation_id: None,
        attempt_id: None,
        kind: SpanKind::Cleanup,
        started_at: MonotonicTimestamp::new(unsafe_id + 4).unwrap(),
        finished_at: MonotonicTimestamp::new(unsafe_id + 5).unwrap(),
        outcome: SpanOutcome::Cleanup {
            error_count: unsafe_id + 6,
            panicking: true,
        },
        correlation: correlation.clone(),
    };
    let bundle = RunTraceBundle {
        run_id,
        compile_id: correlation.compile_id,
        graph_path: correlation.graph_path.clone(),
        selection_digest: correlation.selection_digest.clone(),
        incident_id: Some("incident-public-id".into()),
        metadata: TraceBundleMetadata {
            provenance_scopes: vec![TraceProvenanceScope::from(&correlation)].into_boxed_slice(),
            truncated: true,
            dropped_span_count: unsafe_id + 8,
            estimated_bytes: unsafe_id + 9,
        },
        spans: vec![root, cleanup].into_boxed_slice(),
    };

    let value = serde_json::to_value(TraceBundleDto::from(bundle)).unwrap();
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        keys,
        std::collections::BTreeSet::from([
            "bundleKind".into(),
            "compileId".into(),
            "graphPath".into(),
            "incidentId".into(),
            "metadata".into(),
            "runId".into(),
            "selectionDigest".into(),
            "spans".into(),
        ])
    );
    assert_eq!(value["bundleKind"], "run");
    assert_eq!(value["runId"], unsafe_id.to_string());
    assert_eq!(value["compileId"], unsafe_id.to_string());
    assert_eq!(value["graphPath"], "events/main.yssbi-event");
    assert_eq!(value["incidentId"], "incident-public-id");
    assert_eq!(value["metadata"]["truncated"], true);
    assert_eq!(
        value["metadata"]["droppedSpanCount"],
        (unsafe_id + 8).to_string()
    );
    assert_eq!(
        value["metadata"]["estimatedBytes"],
        (unsafe_id + 9).to_string()
    );
    assert_eq!(
        value["metadata"]["provenanceScopes"][0]["compileId"],
        unsafe_id.to_string()
    );
    assert_eq!(value["spans"][1]["spanId"], (unsafe_id + 3).to_string());
    assert_eq!(
        value["spans"][1]["outcome"]["cleanup"]["errorCount"],
        (unsafe_id + 6).to_string()
    );
    assert_eq!(
        value["spans"][1]["correlation"]["nodeId"],
        node_id.to_string()
    );
    assert!(
        value["spans"][1]["correlation"]
            .get("traceParentSpanId")
            .is_none()
    );
}

#[test]
fn command_trace_maps_not_found_without_echoing_run_id() {
    let root = std::env::temp_dir().join(format!("yssbi-command-trace-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let instance_id = state.capture_project_session().unwrap().instance_id;

    let error =
        get_run_trace_bundle_from_state(&state, instance_id, "9007199254740993".to_string())
            .unwrap_err();

    assert_eq!(error.code(), "trace_not_found");
    assert!(error.details().is_none());
    let wire = serde_json::to_string(&error).unwrap();
    assert!(!wire.contains("message"));
    assert!(!wire.contains("9007199254740993"));
}

#[test]
fn command_trace_maps_stale_project_without_internal_lifecycle_message() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-command-trace-stale-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let stale = state.capture_project_session().unwrap().instance_id;
    state.activate_project_fixture("command-trace-replacement".into(), ProjectData::new());

    let error = get_run_trace_bundle_from_state(&state, stale, "7".to_string()).unwrap_err();

    assert_eq!(error.code(), "trace_project_stale");
    assert!(error.details().is_none());
    assert!(error.incident_id().is_none());
}

#[test]
fn command_trace_rejects_non_decimal_or_zero_run_id() {
    let state = ProjectState::new();
    for run_id in ["7.0", "0", "-1", "+1", "01"] {
        let error = get_run_trace_bundle_from_state(
            &state,
            crate::project::ProjectInstanceId::new(),
            run_id.to_string(),
        )
        .unwrap_err();
        assert_eq!(error.code(), "invalid_opaque_id");
        assert!(error.details().is_none());
    }
}
