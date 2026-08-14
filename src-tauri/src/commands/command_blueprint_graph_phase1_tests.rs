use super::command_node_system::mutate_graph_document_with_emitter;
use crate::event::{Event, EventProject};
use crate::node_system::testing::blueprint_phase1::{
    BlueprintPhase1Fixture, PHASE1_COMPLEX_MUTATIONS,
};

#[test]
fn command_blueprint_graph_phase1_tests_success_emits_exactly_one_complete_graph_delta() {
    for kind in PHASE1_COMPLEX_MUTATIONS {
        let fixture = BlueprintPhase1Fixture::new(kind);
        let request = fixture.success_request(kind);
        let operation_id = request.operation_id;
        let from_revision = request.base_revision;
        let mut events = Vec::new();

        let result = mutate_graph_document_with_emitter(
            &fixture.state,
            fixture.project_instance_id.clone(),
            fixture.graph_path.as_str().to_owned(),
            "en-US",
            serde_json::to_value(request).unwrap(),
            |event| events.push(event),
        )
        .unwrap_or_else(|error| panic!("{} command success failed: {error:?}", kind.label()));

        assert_eq!(events.len(), 1, "{} event count", kind.label());
        let Event::Project(EventProject::GraphDelta {
            project_instance_id,
            delta,
        }) = &events[0]
        else {
            panic!("{} emitted a non-GraphDelta event", kind.label());
        };
        assert_eq!(project_instance_id, fixture.project_instance_id.as_str());
        assert_eq!(delta.graph_path.0.as_ref(), fixture.graph_path.as_str());
        assert_eq!(delta.from_revision, from_revision);
        assert_eq!(delta.to_revision, from_revision.next());
        assert_eq!(delta.caused_by, Some(operation_id));
        assert_eq!(delta, &result.delta);
        assert_eq!(delta.payload, result.delta.payload);
        let mut expected_operations = fixture.expected_success_operations(kind);
        if matches!(
            kind,
            crate::node_system::testing::blueprint_phase1::Phase1ComplexMutation::ConnectReplacement
                | crate::node_system::testing::blueprint_phase1::Phase1ComplexMutation::MoveConnections
        ) {
            let actual_ids = delta
                .payload
                .operations
                .iter()
                .filter_map(|operation| match operation {
                    crate::node_system::document::GraphDocumentOperation::InsertConnection {
                        connection,
                    } => Some(connection.id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected_ids = expected_operations
                .iter_mut()
                .filter_map(|operation| match operation {
                    crate::node_system::document::GraphDocumentOperation::InsertConnection {
                        connection,
                    } => Some(&mut connection.id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(actual_ids.len(), expected_ids.len());
            for (expected, actual) in expected_ids.into_iter().zip(actual_ids.iter().copied()) {
                *expected = actual;
            }
            if kind
                == crate::node_system::testing::blueprint_phase1::Phase1ComplexMutation::MoveConnections
            {
                let removed_ids = delta
                    .payload
                    .operations
                    .iter()
                    .filter_map(|operation| match operation {
                        crate::node_system::document::GraphDocumentOperation::RemoveConnection {
                            connection,
                        } => Some(connection.id),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert!(actual_ids.iter().all(|id| !removed_ids.contains(id)));
            }
        }
        assert_eq!(
            delta.payload.operations,
            expected_operations,
            "{} exact command patch",
            kind.label()
        );
        if kind
            == crate::node_system::testing::blueprint_phase1::Phase1ComplexMutation::MoveConnections
        {
            assert_eq!(fixture.connection_id_allocation_count(), 0);
        }
    }
}

#[test]
fn command_blueprint_graph_phase1_tests_validation_failure_emits_zero_events() {
    for kind in PHASE1_COMPLEX_MUTATIONS {
        let fixture = BlueprintPhase1Fixture::new(kind);
        let before = fixture.authority_snapshot();
        let mut events = Vec::new();

        let error = mutate_graph_document_with_emitter(
            &fixture.state,
            fixture.project_instance_id.clone(),
            fixture.graph_path.as_str().to_owned(),
            "en-US",
            serde_json::to_value(fixture.validation_failure_request(kind)).unwrap(),
            |event| events.push(event),
        )
        .expect_err("validation failure must not return a command result");

        assert_eq!(
            error.code,
            kind.validation_error_code(),
            "{} code",
            kind.label()
        );
        assert!(
            events.is_empty(),
            "{} emitted on validation failure",
            kind.label()
        );
        assert_eq!(fixture.authority_snapshot(), before);
        if kind
            == crate::node_system::testing::blueprint_phase1::Phase1ComplexMutation::MoveConnections
        {
            assert_eq!(fixture.connection_id_allocation_count(), 0);
        }
    }
}

#[test]
fn command_blueprint_graph_phase1_tests_empty_derived_disconnect_emits_zero_events() {
    for kind in [
        crate::node_system::testing::blueprint_phase1::Phase1ComplexMutation::DisconnectPort,
        crate::node_system::testing::blueprint_phase1::Phase1ComplexMutation::DisconnectNode,
    ] {
        let fixture = BlueprintPhase1Fixture::new(kind);
        let before = fixture.authority_snapshot();
        let mut events = Vec::new();
        let result = mutate_graph_document_with_emitter(
            &fixture.state,
            fixture.project_instance_id.clone(),
            fixture.graph_path.as_str().to_owned(),
            "en-US",
            serde_json::to_value(fixture.empty_derived_disconnect_request()).unwrap(),
            |event| events.push(event),
        )
        .unwrap();

        assert!(result.delta.payload.operations.is_empty());
        assert_eq!(result.delta.from_revision, result.delta.to_revision);
        assert!(events.is_empty());
        assert_eq!(fixture.authority_snapshot(), before);
    }
}

#[test]
fn command_blueprint_graph_phase1_tests_stale_emits_zero_events() {
    for kind in PHASE1_COMPLEX_MUTATIONS {
        let fixture = BlueprintPhase1Fixture::new(kind);
        let before = fixture.authority_snapshot();
        let mut events = Vec::new();

        let error = mutate_graph_document_with_emitter(
            &fixture.state,
            fixture.project_instance_id.clone(),
            fixture.graph_path.as_str().to_owned(),
            "en-US",
            serde_json::to_value(fixture.stale_request(kind)).unwrap(),
            |event| events.push(event),
        )
        .expect_err("stale request must not return a command result");

        assert_eq!(
            error.code,
            "graph_revision_conflict",
            "{} code",
            kind.label()
        );
        assert!(
            events.is_empty(),
            "{} emitted on stale request",
            kind.label()
        );
        assert_eq!(fixture.authority_snapshot(), before);
        if kind
            == crate::node_system::testing::blueprint_phase1::Phase1ComplexMutation::MoveConnections
        {
            assert_eq!(fixture.connection_id_allocation_count(), 0);
        }
    }
}
