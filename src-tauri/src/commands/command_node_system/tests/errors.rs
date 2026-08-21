use super::*;

#[test]
fn phase1_error_protocol_editor_rejections_are_safe_and_stable() {
    use crate::node_system::document::{
        EditorMutationError, EditorMutationErrorCode, MutationConflict,
    };

    let detail = "port events/Main:node/00000000-0000-0000-0000-000000000123/data_out";
    let cases = [
        (
            EditorMutationErrorCode::GraphPortNotFound,
            "graph_port_not_found",
        ),
        (
            EditorMutationErrorCode::GraphNodeNotFound,
            "graph_node_not_found",
        ),
        (
            EditorMutationErrorCode::GraphConnectionNotFound,
            "graph_connection_not_found",
        ),
        (
            EditorMutationErrorCode::GraphPortOrphan,
            "graph_port_orphan",
        ),
        (
            EditorMutationErrorCode::GraphConnectionDirectionMismatch,
            "graph_connection_direction_mismatch",
        ),
        (
            EditorMutationErrorCode::GraphConnectionKindMismatch,
            "graph_connection_kind_mismatch",
        ),
        (
            EditorMutationErrorCode::GraphConnectionTypeMismatch,
            "graph_connection_type_mismatch",
        ),
        (
            EditorMutationErrorCode::GraphConnectionTypeUnavailable,
            "graph_connection_type_unavailable",
        ),
        (
            EditorMutationErrorCode::GraphConnectionTypeUnresolved,
            "graph_connection_type_unresolved",
        ),
        (
            EditorMutationErrorCode::GraphConnectionLimitReached,
            "graph_connection_limit_reached",
        ),
        (
            EditorMutationErrorCode::GraphConnectionOrderRequired,
            "graph_connection_order_required",
        ),
        (
            EditorMutationErrorCode::GraphConnectionOrderForbidden,
            "graph_connection_order_forbidden",
        ),
        (
            EditorMutationErrorCode::GraphConnectionAlreadyExists,
            "graph_connection_already_exists",
        ),
        (
            EditorMutationErrorCode::GraphConnectionMoveSourceEmpty,
            "graph_connection_move_source_empty",
        ),
        (
            EditorMutationErrorCode::GraphConnectionMoveSamePort,
            "graph_connection_move_same_port",
        ),
        (
            EditorMutationErrorCode::GraphMutationEmptyTargets,
            "graph_mutation_empty_targets",
        ),
        (
            EditorMutationErrorCode::GraphMutationDuplicateTarget,
            "graph_mutation_duplicate_target",
        ),
        (
            EditorMutationErrorCode::GraphManagedNodeDeleteForbidden,
            "graph_managed_node_delete_forbidden",
        ),
    ];

    for (code, expected_code) in cases {
        let error = mutation_conflict_to_command_error(
            MutationConflict::Editor(EditorMutationError {
                code,
                detail: detail.into(),
            }),
            "graph_revision_conflict",
        );
        let serialized = serde_json::to_value(error).unwrap();
        assert_eq!(serialized["code"], expected_code);
        assert_eq!(serialized.as_object().unwrap().len(), 3);
        assert!(serialized.get("message").is_none());
        assert_eq!(
            serialized["details"],
            serde_json::json!({ "category": "graphMutation" })
        );
        assert!(serialized["incidentId"].is_null());
        let wire = serialized.to_string();
        assert!(!wire.contains("00000000-0000-0000-0000-000000000123"));
        assert!(!wire.contains("events/Main"));
        assert!(!wire.contains("data_out"));
    }
}

#[test]
fn phase1_error_protocol_stale_revision_is_safe_and_stable() {
    let error = mutation_conflict_to_command_error(
        crate::node_system::document::MutationConflict::StaleRevision {
            base_revision: ResourceRevision::new(4),
            current_revision: ResourceRevision::new(5),
        },
        "graph_revision_conflict",
    );

    assert_eq!(
        serde_json::to_value(error).unwrap(),
        serde_json::json!({
            "code": "graph_revision_conflict",
            "details": { "category": "graphMutation" },
            "incidentId": null
        })
    );
}

#[test]
fn phase1_error_protocol_unexpected_conflicts_remain_internal() {
    use crate::node_system::document::{DocumentError, MutationConflict, NodeId};

    for conflict in [
        MutationConflict::Projection("projection detail".into()),
        MutationConflict::History("history detail".into()),
        MutationConflict::Document(DocumentError::NodeNotFound(NodeId::from_uuid(
            uuid::Uuid::from_u128(123),
        ))),
    ] {
        let error = mutation_conflict_to_command_error(conflict, "graph_revision_conflict");
        assert_eq!(error.code(), "internal_error");
        assert!(error.details().is_none());
        assert!(error.incident_id().is_some());
    }
}

#[test]
fn recovery_mutation_conflict_preserves_stable_command_error_code() {
    let error = mutation_conflict_to_command_error(
        crate::node_system::document::MutationConflict::RecoveryRequired(
            "project requires recovery".into(),
        ),
        "graph_revision_conflict",
    );

    assert_eq!(error.code(), "project_recovery_required");
    assert_eq!(
        error.details(),
        serde_json::json!({ "recoveryRequired": true }).as_object()
    );
}
