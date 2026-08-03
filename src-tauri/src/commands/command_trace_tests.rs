use super::command_trace::{TraceRecordDto, get_run_trace_from_state};
use crate::node_system::analysis::{
    CompileId, CorrelationContext, ParentCallId, ProjectSessionId, ResourceKey, ResourceVersion,
    RunId, SpanEvent, SpanKind, SpanStatus, TraceRecord, TraceValue,
};
use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
use crate::node_system::protocol::NodeTypeId;
use crate::node_system::registry::RegistryFingerprint;
use crate::project::{ProjectData, ProjectState};
use std::collections::BTreeMap;

#[test]
fn command_trace_dto_uses_decimal_ids_full_correlation_and_field_allowlist() {
    let unsafe_id = 9_007_199_254_740_993_u64;
    let node_id = NodeId::new();
    let event = SpanEvent {
        kind: SpanKind::RelationalBackend,
        status: SpanStatus::Failed,
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
            run_id: Some(RunId::new(unsafe_id)),
            node_id: Some(node_id),
            node_type_id: Some(NodeTypeId::new("yssbi.test.node").unwrap()),
            parent_call: Some(ParentCallId::new(unsafe_id)),
        },
        fields: BTreeMap::from([
            ("backendId".into(), TraceValue::Redacted),
            ("subplanIndex".into(), TraceValue::Integer(4)),
            (
                "rawError".into(),
                TraceValue::Text("database password leaked".into()),
            ),
            (
                "rows".into(),
                TraceValue::Text("customer row leaked".into()),
            ),
        ]),
    };

    let second_event = SpanEvent {
        correlation: CorrelationContext {
            selection_digest: Some("demand-selection-b".into()),
            ..event.correlation.clone()
        },
        ..event.clone()
    };
    let value = serde_json::to_value(TraceRecordDto::from(TraceRecord {
        sequence: unsafe_id,
        event,
    }))
    .unwrap();
    let second = serde_json::to_value(TraceRecordDto::from(TraceRecord {
        sequence: unsafe_id + 1,
        event: second_event,
    }))
    .unwrap();

    assert_eq!(value["sequence"], unsafe_id.to_string());
    assert_eq!(value["kind"], "relationalBackend");
    assert_eq!(value["status"], "failed");
    assert_eq!(value["correlation"]["projectSessionId"], "session-7");
    assert_eq!(value["correlation"]["graphPath"], "events/main.yssbi-event");
    assert_eq!(value["correlation"]["graphRevision"], unsafe_id.to_string());
    assert_eq!(value["correlation"]["compileId"], unsafe_id.to_string());
    assert_eq!(
        value["correlation"]["selectionDigest"],
        "demand-selection-a"
    );
    assert!(value["correlation"].get("selection_digest").is_none());
    assert_eq!(
        value["correlation"]["compileId"],
        second["correlation"]["compileId"]
    );
    assert_ne!(
        value["correlation"]["selectionDigest"],
        second["correlation"]["selectionDigest"]
    );
    assert_eq!(value["correlation"]["runId"], unsafe_id.to_string());
    assert_eq!(value["correlation"]["nodeId"], node_id.to_string());
    assert_eq!(value["correlation"]["nodeTypeId"], "yssbi.test.node");
    assert_eq!(value["correlation"]["parentCall"], unsafe_id.to_string());
    assert_eq!(
        value["correlation"]["resourceVersions"]["functions/shared"],
        "9"
    );
    assert_eq!(
        value["fields"]["backendId"],
        serde_json::json!({ "type": "redacted" })
    );
    assert_eq!(
        value["fields"]["subplanIndex"],
        serde_json::json!({ "type": "integer", "value": 4 })
    );
    assert!(value["fields"].get("rawError").is_none());
    assert!(value["fields"].get("rows").is_none());
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(!serialized.contains("database password leaked"));
    assert!(!serialized.contains("customer row leaked"));
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
    assert!(!error.message.contains("project instance"));
    assert!(error.details.is_none());
}

#[test]
fn command_trace_rejects_non_decimal_run_id() {
    let state = ProjectState::new();
    let error = get_run_trace_from_state(
        &state,
        crate::project::ProjectInstanceId::new(),
        "7.0".to_string(),
    )
    .unwrap_err();

    assert_eq!(error.code, "invalid_opaque_id");
    assert_eq!(error.message, "runId must be an unsigned decimal string.");
}
