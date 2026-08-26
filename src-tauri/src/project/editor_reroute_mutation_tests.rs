use super::*;
use crate::event::GraphMutationResultDto;
use crate::graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, GraphRevision, NodeId,
    NodePosition, OrderKey, ParameterValues, PortAddress,
};
use crate::node_system::catalog::{DATA_REROUTE_NODE_TYPE, build_builtin_node_system};
use crate::node_system::document::{
    EditorGraphMutationDto, GraphDocumentOperation, HistoryMutation, MutationConflict,
    MutationRequest, ResourceKey,
};
use crate::node_system::protocol::{NodeTypeId, PortKey};
use crate::project::{OperationId, ResourceRevision};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};

const ORIGINAL_ID: u128 = 0x5101;
const SOURCE_ID: u128 = 0x5201;
const TARGET_ID: u128 = 0x5202;

#[test]
fn phase2_reroute_authority_success_commits_one_complete_publication() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let observed = Mutex::new(Vec::new());
    let result = fixture
        .state
        .apply_editor_graph_mutation_observed(
            &fixture.project_instance_id,
            &fixture.graph_path,
            "en-US",
            fixture.request(before.revision, 1),
            |delta| observed.lock().unwrap().push(delta.clone()),
        )
        .unwrap();
    let after = fixture.snapshot();

    assert_eq!(result.delta.from_revision, before.revision);
    assert_eq!(
        result.delta.to_revision.get(),
        result.delta.from_revision.get() + 1
    );
    assert_eq!(after.revision, before.revision.next());
    assert_eq!(
        after.history_lengths,
        (before.history_lengths.0 + 1, before.history_lengths.1)
    );
    assert_eq!(observed.into_inner().unwrap(), vec![result.delta.clone()]);
    assert_eq!(
        result.projection_replacement.projection.source_revision,
        result.delta.to_revision.get()
    );
    assert_eq!(result.delta.payload.operations.len(), 4);
    assert!(matches!(
        result.delta.payload.operations[0],
        GraphDocumentOperation::RemoveConnection { .. }
    ));
    assert!(matches!(
        result.delta.payload.operations[1],
        GraphDocumentOperation::InsertNode { .. }
    ));
    assert!(matches!(
        result.delta.payload.operations[2],
        GraphDocumentOperation::InsertConnection { .. }
    ));
    assert!(matches!(
        result.delta.payload.operations[3],
        GraphDocumentOperation::InsertConnection { .. }
    ));
    assert_eq!(
        after
            .document
            .nodes
            .values()
            .filter(|node| node.node_type.as_str() == DATA_REROUTE_NODE_TYPE)
            .count(),
        1
    );
    assert_eq!(after.document.connections.len(), 2);
    assert_eq!(result.projection_replacement.projection.nodes.len(), 3);
    assert_eq!(
        result.projection_replacement.projection.connections.len(),
        2
    );
    let mut reconstructed = before.document.clone();
    reconstructed.apply_patch(&result.delta.payload).unwrap();
    assert_document_content_eq(&reconstructed, &after.document);
}

#[test]
fn phase2_reroute_authority_failure_has_zero_side_effects_and_zero_observer_calls() {
    for mutation in [
        EditorGraphMutationDto::InsertReroute {
            connection_id: connection_id(0xffff),
            position: NodePosition { x: 10.0, y: 20.0 },
        },
        EditorGraphMutationDto::InsertReroute {
            connection_id: connection_id(ORIGINAL_ID),
            position: NodePosition {
                x: f64::NAN,
                y: 20.0,
            },
        },
    ] {
        let fixture = Fixture::new();
        let before = fixture.snapshot();
        let observer_count = std::cell::Cell::new(0);
        let request = fixture.request_with(before.revision, 2, mutation);
        assert!(
            fixture
                .state
                .apply_editor_graph_mutation_observed(
                    &fixture.project_instance_id,
                    &fixture.graph_path,
                    "en-US",
                    request,
                    |_| observer_count.set(observer_count.get() + 1),
                )
                .is_err()
        );
        assert_eq!(observer_count.get(), 0);
        assert_eq!(fixture.snapshot(), before);
    }
}

#[test]
fn phase2_reroute_authority_stale_has_zero_side_effects_and_zero_observer_calls() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let observer_count = std::cell::Cell::new(0);
    let error = fixture
        .state
        .apply_editor_graph_mutation_observed(
            &fixture.project_instance_id,
            &fixture.graph_path,
            "en-US",
            fixture.request(before.revision.next(), 3),
            |_| observer_count.set(observer_count.get() + 1),
        )
        .unwrap_err();
    assert!(matches!(error, MutationConflict::StaleRevision { .. }));
    assert_eq!(observer_count.get(), 0);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn phase2_reroute_authority_history_undo_redo_restores_exact_identity() {
    let fixture = Fixture::new();
    let original = fixture.snapshot();
    let committed = fixture
        .state
        .apply_editor_graph_mutation(
            &fixture.project_instance_id,
            &fixture.graph_path,
            "en-US",
            fixture.request(original.revision, 4),
        )
        .unwrap();
    let committed_snapshot = fixture.snapshot();
    let committed_ids = inserted_ids(&committed);

    let undo_observers = AtomicUsize::new(0);
    let undo = fixture
        .state
        .undo_last_transaction_observed(
            &fixture.project_instance_id,
            "en-US",
            fixture.history_request(committed_snapshot.revision, 5),
            |_| {
                undo_observers.fetch_add(1, Ordering::SeqCst);
            },
        )
        .unwrap();
    assert_eq!(undo_observers.load(Ordering::SeqCst), 1);
    assert_eq!(undo.deltas.len(), 1);
    let undone = fixture.snapshot();
    assert_document_content_eq(&undone.document, &original.document);
    assert_eq!(
        undone.document.connections[&connection_id(ORIGINAL_ID)].order,
        Some(OrderKey::new("original-order"))
    );
    assert_eq!(undone.history_lengths, (0, 1));
    assert_eq!(undone.revision, committed_snapshot.revision.next());

    let redo_observers = AtomicUsize::new(0);
    let redo = fixture
        .state
        .redo_last_transaction_observed(
            &fixture.project_instance_id,
            "en-US",
            fixture.history_request(undone.revision, 6),
            |_| {
                redo_observers.fetch_add(1, Ordering::SeqCst);
            },
        )
        .unwrap();
    assert_eq!(redo_observers.load(Ordering::SeqCst), 1);
    assert_eq!(redo.deltas.len(), 1);
    let redone = fixture.snapshot();
    assert_document_content_eq(&redone.document, &committed_snapshot.document);
    assert_eq!(inserted_document_ids(&redone.document), committed_ids);
    assert_eq!(redone.history_lengths, (1, 0));
    assert_eq!(redone.revision, undone.revision.next());
}

#[test]
fn phase2_reroute_authority_projection_failure_rolls_back_before_observer() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let fail_once = Arc::new(AtomicBool::new(true));
    let fail_for_hook = Arc::clone(&fail_once);
    fixture.state.set_projection_test_hook(Arc::new(move || {
        if fail_for_hook.swap(false, Ordering::SeqCst) {
            Err("injected reroute projection failure".into())
        } else {
            Ok(())
        }
    }));
    let observer_count = AtomicUsize::new(0);
    let error = fixture
        .state
        .apply_editor_graph_mutation_observed(
            &fixture.project_instance_id,
            &fixture.graph_path,
            "en-US",
            fixture.request(before.revision, 9),
            |_| {
                observer_count.fetch_add(1, Ordering::SeqCst);
            },
        )
        .unwrap_err();
    assert!(matches!(error, MutationConflict::Projection(_)));
    assert_eq!(observer_count.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn phase2_reroute_authority_concurrency_commits_only_one_winner() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let captured = Arc::new(Barrier::new(3));
    let hook_calls = Arc::new(AtomicUsize::new(0));
    let captured_for_hook = Arc::clone(&captured);
    let hook_calls_for_hook = Arc::clone(&hook_calls);
    fixture
        .state
        .set_mutation_publication_test_hook(Arc::new(move || {
            hook_calls_for_hook.fetch_add(1, Ordering::SeqCst);
            captured_for_hook.wait();
        }));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let outcomes = std::thread::scope(|scope| {
        let handles = [7_u128, 8_u128].map(|flavor| {
            let observed = Arc::clone(&observed);
            let fixture = &fixture;
            scope.spawn(move || {
                fixture.state.apply_editor_graph_mutation_observed(
                    &fixture.project_instance_id,
                    &fixture.graph_path,
                    "en-US",
                    fixture.request(before.revision, flavor),
                    |delta| observed.lock().unwrap().push(delta.clone()),
                )
            })
        });
        captured.wait();
        handles.map(|handle| handle.join().unwrap())
    });

    assert_eq!(hook_calls.load(Ordering::SeqCst), 2);
    let winners = outcomes
        .iter()
        .filter_map(|outcome| outcome.as_ref().ok())
        .collect::<Vec<_>>();
    let stale = outcomes
        .iter()
        .filter(|result| matches!(result, Err(MutationConflict::StaleRevision { .. })))
        .count();
    assert_eq!(winners.len(), 1);
    assert_eq!(stale, 1);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[winners[0].delta.clone()]
    );
    let after = fixture.snapshot();
    assert_eq!(after.revision, before.revision.next());
    assert_eq!(after.history_lengths, (1, 0));
    assert_eq!(after.document.nodes.len(), 3);
    assert_eq!(after.document.connections.len(), 2);
    assert_eq!(
        after.projection,
        winners[0].projection_replacement.projection
    );
}

#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    document: GraphDocument,
    revision: ResourceRevision,
    history_lengths: (usize, usize),
    history_heads: (
        Option<crate::project::HistoryEntryId>,
        Option<crate::project::HistoryEntryId>,
    ),
    publication: (String, u64, u64),
    projection: crate::node_system::analysis::EditorGraphProjectionDto,
    complete_authority: serde_json::Value,
}

struct Fixture {
    state: ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    _project: fixtures::TempProject,
}

impl Fixture {
    fn new() -> Self {
        let project =
            fixtures::TempProject::activate("phase2-reroute-authority", ProjectData::new());
        let state = project.state().clone();
        state.project_store.write().unwrap().node_registry =
            build_builtin_node_system().unwrap().registry;
        let graph_path = GraphResourcePath::new("events/Phase2Reroute.yssbi-event").unwrap();
        let mut graph = GraphResourceDocument::new("Phase 2 Reroute", GraphDocumentKind::Event);
        graph.document = base_document();
        state.insert_graph(graph_path.clone(), graph).unwrap();
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        Self {
            state,
            project_instance_id,
            graph_path,
            _project: project,
        }
    }

    fn request(
        &self,
        revision: ResourceRevision,
        flavor: u128,
    ) -> MutationRequest<EditorGraphMutationDto> {
        self.request_with(
            revision,
            flavor,
            EditorGraphMutationDto::InsertReroute {
                connection_id: connection_id(ORIGINAL_ID),
                position: NodePosition {
                    x: 100.0 + flavor as f64,
                    y: 40.0,
                },
            },
        )
    }

    fn request_with(
        &self,
        revision: ResourceRevision,
        flavor: u128,
        payload: EditorGraphMutationDto,
    ) -> MutationRequest<EditorGraphMutationDto> {
        MutationRequest::new(
            self.resource_key(),
            revision,
            OperationId::from_uuid(uuid::Uuid::from_u128(0x6000 + flavor)),
            payload,
        )
    }

    fn history_request(
        &self,
        revision: ResourceRevision,
        flavor: u128,
    ) -> MutationRequest<HistoryMutation> {
        MutationRequest::new(
            self.resource_key(),
            revision,
            OperationId::from_uuid(uuid::Uuid::from_u128(0x7000 + flavor)),
            HistoryMutation {},
        )
    }

    fn resource_key(&self) -> ResourceKey {
        ResourceKey::Graph(self.graph_path.clone())
    }

    fn snapshot(&self) -> Snapshot {
        let document = self.state.get_data().unwrap().graphs[&self.graph_path]
            .document
            .clone();
        Snapshot {
            revision: ResourceRevision::from_graph_revision(document.revision),
            document,
            history_lengths: self.state.history_lengths_for_test(),
            history_heads: (
                self.state.history_head_id_for_test(true),
                self.state.history_head_id_for_test(false),
            ),
            publication: self.state.publication_state_for_test(),
            projection: self
                .state
                .graph_projection_for_project(&self.project_instance_id, &self.graph_path, "en-US")
                .unwrap(),
            complete_authority: serde_json::json!({
                "data": self.state.get_data().unwrap(),
                "history": self.state.history_status(),
                "historyLengths": self.state.history_lengths_for_test(),
                "historyHeads": [
                    self.state.history_head_id_for_test(true),
                    self.state.history_head_id_for_test(false),
                ],
                "revisions": self.state.revision_state_for_test(),
                "publication": self.state.publication_state_for_test(),
            }),
        }
    }
}

fn base_document() -> GraphDocument {
    let mut document = GraphDocument::default();
    for (id, node_type, x) in [
        (SOURCE_ID, "yssbi.constant.int64", 0.0),
        (TARGET_ID, "yssbi.debug.view", 240.0),
    ] {
        document.nodes.insert(
            node_id(id),
            DocumentNode {
                id: node_id(id),
                node_type: NodeTypeId::new(node_type).unwrap(),
                position: NodePosition { x, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    let connection = DocumentConnection {
        id: connection_id(ORIGINAL_ID),
        output: declared(node_id(SOURCE_ID), "value"),
        input: declared(node_id(TARGET_ID), "data"),
        order: Some(OrderKey::new("original-order")),
    };
    document.connections.insert(connection.id, connection);
    document.revision = GraphRevision::INITIAL;
    document
}

fn inserted_ids(result: &GraphMutationResultDto) -> (NodeId, Vec<ConnectionId>) {
    let node = result
        .delta
        .payload
        .operations
        .iter()
        .find_map(|operation| match operation {
            GraphDocumentOperation::InsertNode { node } => Some(node.id),
            _ => None,
        })
        .unwrap();
    let mut connections = result
        .delta
        .payload
        .operations
        .iter()
        .filter_map(|operation| match operation {
            GraphDocumentOperation::InsertConnection { connection } => Some(connection.id),
            _ => None,
        })
        .collect::<Vec<_>>();
    connections.sort();
    (node, connections)
}

fn inserted_document_ids(document: &GraphDocument) -> (NodeId, Vec<ConnectionId>) {
    let node = document
        .nodes
        .values()
        .find(|node| node.node_type.as_str() == DATA_REROUTE_NODE_TYPE)
        .unwrap()
        .id;
    (node, document.connections.keys().copied().collect())
}

fn assert_document_content_eq(actual: &GraphDocument, expected: &GraphDocument) {
    let mut actual = actual.clone();
    actual.revision = expected.revision;
    assert_eq!(actual, *expected);
}

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(uuid::Uuid::from_u128(value))
}
fn connection_id(value: u128) -> ConnectionId {
    ConnectionId::from_uuid(uuid::Uuid::from_u128(value))
}
fn declared(node: NodeId, key: &str) -> PortAddress {
    PortAddress::declared(node, PortKey::new(key).unwrap())
}
