use crate::event::GraphMutationResultDto;
use crate::node_system::document::{
    DocumentConnection, GraphDocument, HistoryMutation, MutationConflict, MutationRequest,
    OperationId,
};
use crate::node_system::testing::blueprint_phase1::{
    BlueprintPhase1Fixture, PHASE1_COMPLEX_MUTATIONS, Phase1AuthoritySnapshot,
    Phase1ComplexMutation,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Barrier};

#[test]
fn blueprint_graph_phase1_tests_success_authority_history_delta_and_projection_matrix() {
    for kind in PHASE1_COMPLEX_MUTATIONS {
        let fixture = BlueprintPhase1Fixture::new(kind);
        let before = fixture.authority_snapshot();
        let request = fixture.success_request(kind);
        let operation_id = request.operation_id;

        let result = fixture
            .apply_editor_graph_mutation(request)
            .unwrap_or_else(|error| panic!("{} success failed: {error}", kind.label()));
        let committed = fixture.authority_snapshot();

        assert_success_commit(
            kind,
            &before,
            &committed,
            &result,
            operation_id,
            &fixture.expected_success_operations(kind),
        );
        if kind == Phase1ComplexMutation::MoveConnections {
            assert_eq!(fixture.connection_id_allocation_count(), 2);
        }
        assert_one_step_undo_redo(kind, &fixture, &before.document, &committed.document);
    }
}

#[test]
fn blueprint_graph_phase1_tests_validation_failure_preserves_every_authority_surface() {
    for kind in PHASE1_COMPLEX_MUTATIONS {
        let fixture = BlueprintPhase1Fixture::new(kind);
        let before = fixture.authority_snapshot();

        let error = fixture
            .apply_editor_graph_mutation(fixture.validation_failure_request(kind))
            .expect_err("validation failure must not return a delta/result");

        assert_eq!(
            mutation_code(&error),
            kind.validation_error_code(),
            "{} validation code",
            kind.label()
        );
        assert_eq!(
            fixture.authority_snapshot(),
            before,
            "{} changed authority",
            kind.label()
        );
        if kind == Phase1ComplexMutation::MoveConnections {
            assert_eq!(fixture.connection_id_allocation_count(), 0);
        }
    }
}

#[test]
fn blueprint_graph_phase1_tests_stale_and_duplicate_preserve_every_authority_surface() {
    for kind in PHASE1_COMPLEX_MUTATIONS {
        let fixture = BlueprintPhase1Fixture::new(kind);
        let before = fixture.authority_snapshot();
        let error = fixture
            .apply_editor_graph_mutation(fixture.stale_request(kind))
            .expect_err("stale request must not return a delta/result");
        assert!(matches!(error, MutationConflict::StaleRevision { .. }));
        assert_eq!(
            fixture.authority_snapshot(),
            before,
            "{} stale changed authority",
            kind.label()
        );
    }

    let fixture = BlueprintPhase1Fixture::new(Phase1ComplexMutation::ConnectReplacement);
    let before = fixture.authority_snapshot();
    let allocations_before = fixture.connection_id_allocation_count();
    let error = fixture
        .apply_editor_graph_mutation(fixture.duplicate_request())
        .expect_err("duplicate endpoint must not return a delta/result");
    assert_eq!(mutation_code(&error), "graph_connection_already_exists");
    assert_eq!(fixture.connection_id_allocation_count(), allocations_before);
    assert_eq!(allocations_before, 0);
    assert_eq!(fixture.authority_snapshot(), before);
}

#[test]
fn blueprint_graph_phase1_tests_empty_derived_disconnect_is_stable_noop() {
    for kind in [
        Phase1ComplexMutation::DisconnectPort,
        Phase1ComplexMutation::DisconnectNode,
    ] {
        let fixture = BlueprintPhase1Fixture::new(kind);
        let before = fixture.authority_snapshot();
        let result = fixture
            .apply_editor_graph_mutation(fixture.empty_derived_disconnect_request())
            .unwrap();

        assert!(result.delta.payload.operations.is_empty());
        assert_eq!(result.delta.from_revision, before.revision);
        assert_eq!(result.delta.to_revision, before.revision);
        assert_eq!(fixture.authority_snapshot(), before);
    }
}

#[test]
fn blueprint_graph_phase1_tests_same_revision_race_commits_only_one_winner() {
    for kind in PHASE1_COMPLEX_MUTATIONS {
        let fixture = BlueprintPhase1Fixture::new(kind);
        let before = fixture.authority_snapshot();
        let [first, second] = fixture.competing_requests(kind);
        let barrier = Arc::new(Barrier::new(3));
        let outcomes = std::thread::scope(|scope| {
            let handles = [first, second].map(|request| {
                let barrier = barrier.clone();
                let fixture = &fixture;
                scope.spawn(move || {
                    barrier.wait();
                    fixture.apply_editor_graph_mutation(request)
                })
            });
            barrier.wait();
            handles.map(|handle| handle.join().expect("race worker panicked"))
        });

        let winners = outcomes
            .iter()
            .filter_map(|outcome| outcome.as_ref().ok())
            .collect::<Vec<_>>();
        let stale = outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Err(MutationConflict::StaleRevision { .. })))
            .count();
        assert_eq!(winners.len(), 1, "{} winner count", kind.label());
        assert_eq!(
            stale,
            1,
            "{} stale loser count; outcomes={outcomes:?}",
            kind.label()
        );

        let winner = winners[0];
        let committed = fixture.authority_snapshot();
        assert_eq!(committed.revision, before.revision.next());
        assert_eq!(committed.history_lengths, (before.history_lengths.0 + 1, 0));
        assert_eq!(
            committed.projection,
            winner.projection_replacement.projection
        );
        assert_patch_is_complete(&before.document, &committed.document, winner);
        assert_eq!(committed.publication.0, before.publication.0);
        assert_eq!(committed.publication.1, before.publication.1);
        assert_eq!(committed.publication.2, before.publication.2 + 1);
    }
}

fn assert_success_commit(
    kind: Phase1ComplexMutation,
    before: &Phase1AuthoritySnapshot,
    committed: &Phase1AuthoritySnapshot,
    result: &GraphMutationResultDto,
    operation_id: OperationId,
    expected_operations: &[crate::node_system::document::GraphDocumentOperation],
) {
    assert_eq!(
        committed.revision,
        before.revision.next(),
        "{} revision",
        kind.label()
    );
    assert_eq!(committed.history_lengths, (before.history_lengths.0 + 1, 0));
    assert_eq!(result.history.can_undo, true);
    assert_eq!(result.history.can_redo, false);
    assert_eq!(result.delta.from_revision, before.revision);
    assert_eq!(result.delta.to_revision, committed.revision);
    assert_eq!(result.delta.caused_by, Some(operation_id));
    assert_eq!(
        result.delta.graph_path.0.as_ref(),
        result.projection_replacement.graph_path
    );
    assert_eq!(
        result.projection_replacement.projection,
        committed.projection
    );
    assert_eq!(
        result.delta.payload.operations,
        expected_operations,
        "{} exact operation sequence",
        kind.label()
    );
    assert_patch_is_complete(&before.document, &committed.document, result);
    assert_eq!(committed.publication.0, before.publication.0);
    assert_eq!(committed.publication.1, before.publication.1);
    assert_eq!(committed.publication.2, before.publication.2 + 1);
}

fn assert_patch_is_complete(
    before: &GraphDocument,
    committed: &GraphDocument,
    result: &GraphMutationResultDto,
) {
    let mut applied = before.clone();
    applied.apply_patch(&result.delta.payload).unwrap();
    assert_eq!(
        &applied, committed,
        "delta must reconstruct committed authority exactly"
    );
}

fn assert_one_step_undo_redo(
    kind: Phase1ComplexMutation,
    fixture: &BlueprintPhase1Fixture,
    original: &GraphDocument,
    committed: &GraphDocument,
) {
    let undo_revision = fixture.authority_snapshot().revision;
    let mut undo_delta = None;
    fixture
        .state
        .undo_last_transaction_observed(
            &fixture.project_instance_id,
            "en-US",
            history_request(fixture, undo_revision, 0xb1ff_0001),
            |delta| undo_delta = Some(delta.clone()),
        )
        .unwrap();
    let expected_forward = crate::node_system::document::GraphDocumentPatch::new(
        fixture.expected_success_operations(kind),
    );
    let undo_delta = undo_delta.unwrap();
    assert_eq!(undo_delta.deltas.len(), 1);
    let crate::node_system::document::ResourceDocumentPatch::Graph(undo_patch) =
        &undo_delta.deltas[0].payload
    else {
        panic!("{} undo emitted a non-graph patch", kind.label());
    };
    assert_eq!(
        undo_patch.operations,
        expected_forward.inverse().operations,
        "{} exact authority inverse sequence",
        kind.label()
    );
    let undone = fixture.authority_snapshot();
    assert_document_content_eq(&undone.document, original);
    assert_ordered_connections_restored(kind, &undone.document, original);
    assert_eq!(undone.history_lengths, (0, 1));

    let mut redo_delta = None;
    fixture
        .state
        .redo_last_transaction_observed(
            &fixture.project_instance_id,
            "en-US",
            history_request(fixture, undone.revision, 0xb1ff_0002),
            |delta| redo_delta = Some(delta.clone()),
        )
        .unwrap();
    let redo_delta = redo_delta.unwrap();
    assert_eq!(redo_delta.deltas.len(), 1);
    let crate::node_system::document::ResourceDocumentPatch::Graph(redo_patch) =
        &redo_delta.deltas[0].payload
    else {
        panic!("{} redo emitted a non-graph patch", kind.label());
    };
    assert_eq!(
        redo_patch.operations,
        expected_forward.operations,
        "{} exact authority redo sequence",
        kind.label()
    );
    let redone = fixture.authority_snapshot();
    assert_document_content_eq(&redone.document, committed);
    assert_ordered_connections_restored(kind, &redone.document, committed);
    assert_eq!(redone.history_lengths, (1, 0));
}

fn history_request(
    fixture: &BlueprintPhase1Fixture,
    revision: crate::node_system::document::ResourceRevision,
    operation_id: u128,
) -> MutationRequest<HistoryMutation> {
    MutationRequest::new(
        fixture.resource_key(),
        revision,
        OperationId::from_uuid(uuid::Uuid::from_u128(operation_id)),
        HistoryMutation {},
    )
}

fn assert_document_content_eq(actual: &GraphDocument, expected: &GraphDocument) {
    let mut actual = actual.clone();
    actual.revision = expected.revision;
    assert_eq!(
        actual, *expected,
        "nodes/connections/IDs/order keys were not restored"
    );
}

fn assert_ordered_connections_restored(
    _kind: Phase1ComplexMutation,
    actual: &GraphDocument,
    expected: &GraphDocument,
) {
    let ordered = |document: &GraphDocument| {
        document
            .connections
            .iter()
            .filter(|(_, connection)| connection.order.is_some())
            .map(|(id, connection)| (*id, connection.clone()))
            .collect::<BTreeMap<_, DocumentConnection>>()
    };
    let expected = ordered(expected);
    if expected.is_empty() {
        return;
    }
    assert_eq!(ordered(actual), expected);
}

#[test]
fn blueprint_graph_phase1_tests_registry_is_valid_bounded_ordered_projection_authority() {
    for kind in PHASE1_COMPLEX_MUTATIONS {
        let fixture = BlueprintPhase1Fixture::new(kind);
        assert!(
            fixture.document_connection_invariants_hold(),
            "{} fixture violates its frozen registry",
            kind.label()
        );
    }
    let fixture = BlueprintPhase1Fixture::new(Phase1ComplexMutation::MoveConnections);
    let contract = fixture.registry_contract();
    assert_eq!(contract.bounded_maximum, Some(2));
    assert!(!contract.bounded_ordered);
    assert_eq!(contract.ordered_maximum, None);
    assert!(contract.ordered_ordered);
    assert_eq!(
        contract.registry_fingerprint,
        contract.projection_registry_fingerprint
    );
    assert_eq!(contract.projected_bounded_maximum, Some(2));
    assert_eq!(contract.full_bounded_current, 2);
    assert_eq!(contract.full_bounded_maximum, Some(2));
    assert!(contract.projected_ordered);
}

fn mutation_code(error: &MutationConflict) -> &'static str {
    match error {
        MutationConflict::Editor(error) => error.code.as_str(),
        MutationConflict::StaleRevision { .. } => "graph_revision_conflict",
        _ => error.code(),
    }
}
