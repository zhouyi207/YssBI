use super::command_trace::{TraceSpanDto, get_run_trace_from_state};
use crate::node_system::analysis::{
    CompileId, CorrelationContext, MonotonicTimestamp, ParentCallId, ProjectSessionId, ResourceKey,
    ResourceVersion, RunId, SpanId, SpanKind, SpanOutcome, TraceSpan,
};
use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
use crate::node_system::plan::{AttemptId, OperationStableId};
use crate::node_system::protocol::NodeTypeId;
use crate::node_system::registry::RegistryFingerprint;
use crate::node_system::runtime::ActivationId;
use crate::project::{ProjectData, ProjectState};
use std::collections::BTreeMap;

#[test]
fn command_trace_dto_uses_exact_completed_span_decimal_wire() {
    let unsafe_id = 9_007_199_254_740_993_u64;
    let node_id = NodeId::new();
    let run_id = RunId::new(unsafe_id);
    let parent_span_id = SpanId::new(unsafe_id + 1).unwrap();
    let operation_id = OperationStableId::from_digest([7; 32]);
    let activation_id = ActivationId::next().unwrap();
    let attempt_id = AttemptId::new(unsafe_id + 2);
    let span = TraceSpan {
        span_id: SpanId::new(unsafe_id + 3).unwrap(),
        parent_span_id: Some(parent_span_id),
        run_id: Some(run_id),
        operation_id: Some(operation_id.clone()),
        activation_id: Some(activation_id),
        attempt_id: Some(attempt_id),
        kind: SpanKind::AdapterIo,
        started_at: MonotonicTimestamp::new(unsafe_id + 4).unwrap(),
        finished_at: MonotonicTimestamp::new(unsafe_id + 5).unwrap(),
        outcome: SpanOutcome::Cleanup {
            error_count: unsafe_id + 6,
            panicking: true,
        },
        correlation: CorrelationContext {
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
            node_type_id: Some(NodeTypeId::new("yssbi.test.node").unwrap()),
            parent_call: Some(ParentCallId::new(unsafe_id)),
            trace_parent_span_id: Some(parent_span_id),
        },
    };

    let value = serde_json::to_value(TraceSpanDto::from(span)).unwrap();
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        keys,
        std::collections::BTreeSet::from([
            "activationId".into(),
            "attemptId".into(),
            "correlation".into(),
            "finishedAt".into(),
            "kind".into(),
            "operationId".into(),
            "outcome".into(),
            "parentSpanId".into(),
            "runId".into(),
            "spanId".into(),
            "startedAt".into(),
        ])
    );
    assert_eq!(value["spanId"], (unsafe_id + 3).to_string());
    assert_eq!(value["parentSpanId"], (unsafe_id + 1).to_string());
    assert_eq!(value["runId"], unsafe_id.to_string());
    assert_eq!(value["operationId"], operation_id.as_str());
    assert_eq!(value["activationId"], activation_id.get().to_string());
    assert_eq!(value["attemptId"], (unsafe_id + 2).to_string());
    assert_eq!(value["kind"], "adapterIo");
    assert_eq!(value["startedAt"], (unsafe_id + 4).to_string());
    assert_eq!(value["finishedAt"], (unsafe_id + 5).to_string());
    assert_eq!(
        value["outcome"]["cleanup"]["errorCount"],
        (unsafe_id + 6).to_string()
    );
    assert_eq!(value["outcome"]["cleanup"]["panicking"], true);
    assert_eq!(value["correlation"]["graphRevision"], unsafe_id.to_string());
    assert_eq!(value["correlation"]["compileId"], unsafe_id.to_string());
    assert_eq!(value["correlation"]["runId"], unsafe_id.to_string());
    assert_eq!(value["correlation"]["parentCall"], unsafe_id.to_string());
    assert!(value["correlation"].get("traceParentSpanId").is_none());
}

#[test]
fn command_trace_maps_not_found_without_echoing_run_id() {
    let root = std::env::temp_dir().join(format!("yssbi-command-trace-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let instance_id = state.capture_project_session().unwrap().instance_id;

    let error =
        get_run_trace_from_state(&state, instance_id, "9007199254740993".to_string()).unwrap_err();

    assert_eq!(error.code, "trace_not_found");
    assert_eq!(error.message, "The requested trace is no longer retained.");
    assert!(!error.message.contains("9007199254740993"));
    assert!(error.details.is_none());
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

    let error = get_run_trace_from_state(&state, stale, "7".to_string()).unwrap_err();

    assert_eq!(error.code, "trace_project_stale");
    assert_eq!(
        error.message,
        "The active project changed; refresh trace details."
    );
    assert!(error.details.is_none());
}

#[test]
fn command_trace_rejects_non_decimal_or_zero_run_id() {
    let state = ProjectState::new();
    for run_id in ["7.0", "0", "-1", "+1", "01"] {
        let error = get_run_trace_from_state(
            &state,
            crate::project::ProjectInstanceId::new(),
            run_id.to_string(),
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_opaque_id");
        assert_eq!(
            error.message,
            "runId must be a non-zero unsigned decimal string."
        );
    }
}
