use super::materialization::ProjectedMemberRef;
use super::*;
use crate::node_system::protocol::{NodeTypeId, PortKey};
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(Uuid::from_u128(value))
}

fn instance_id(value: u128) -> PortInstanceId {
    PortInstanceId::from_uuid(Uuid::from_u128(value))
}

fn connection_id(value: u128) -> ConnectionId {
    ConnectionId::from_uuid(Uuid::from_u128(value))
}

fn operation_id(value: u128) -> OperationId {
    OperationId::from_uuid(Uuid::from_u128(value))
}

fn graph_path(value: &str) -> GraphResourcePath {
    GraphResourcePath(value.into())
}

fn node(id: NodeId) -> DocumentNode {
    DocumentNode {
        id,
        node_type: NodeTypeId::new("yssbi.test.node").unwrap(),
        position: NodePosition { x: 1.0, y: 2.0 },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

fn declared(id: NodeId, key: &str) -> PortAddress {
    PortAddress::declared(id, PortKey::new(key).unwrap())
}

fn assert_graph_content_eq(left: &GraphDocument, right: &GraphDocument) {
    assert_eq!(left.nodes, right.nodes);
    assert_eq!(left.port_bindings, right.port_bindings);
    assert_eq!(left.connections, right.connections);
    assert_eq!(left.input_states, right.input_states);
}

fn binding() -> DynamicPortBinding {
    DynamicPortBinding::Resolved {
        origin: DynamicMemberLocator::SchemaField {
            source: SchemaSourceIdentity("source".into()),
            field: SchemaFieldIdentity("field".into()),
        },
        order: OrderKey("a".into()),
    }
}

#[test]
fn declared_port_address_needs_no_persisted_instance() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();

    document
        .connect(declared(first, "output"), declared(second, "input"), None)
        .unwrap();

    assert!(document.port_bindings.is_empty());
    assert!(document.validate().is_ok());
}

#[test]
fn instance_address_requires_a_binding() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();
    let input = PortAddress::instance(second, PortKey::new("fields").unwrap(), instance_id(10));

    assert!(matches!(
        document.connect(declared(first, "output"), input.clone(), None),
        Err(DocumentError::MissingPortBinding(address)) if address == input
    ));
    assert!(document.connections.is_empty());

    document.bind_port(input.clone(), binding()).unwrap();
    document
        .connect(declared(first, "output"), input, None)
        .unwrap();
}

#[test]
fn deleting_a_node_atomically_removes_owned_and_incident_data() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();
    let input = PortAddress::instance(second, PortKey::new("fields").unwrap(), instance_id(10));
    document.bind_port(input.clone(), binding()).unwrap();
    document
        .set_literal(input.clone(), Some(json!(42)))
        .unwrap();
    document
        .connect(declared(first, "output"), input, None)
        .unwrap();

    document.delete_node(second).unwrap();

    assert!(!document.nodes.contains_key(&second));
    assert!(document.connections.is_empty());
    assert!(document.port_bindings.is_empty());
    assert!(document.input_states.is_empty());
    assert!(document.validate().is_ok());
}

#[test]
fn connections_override_but_do_not_discard_literals() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();
    let input = declared(second, "input");
    document
        .set_literal(input.clone(), Some(json!(42)))
        .unwrap();
    let connection = document
        .connect(declared(first, "output"), input.clone(), None)
        .unwrap();

    assert_eq!(
        document.effective_input_binding(&input, Some(json!(0))),
        EffectiveInputBinding::Connections(vec![connection])
    );
    document.disconnect(connection).unwrap();
    assert_eq!(
        document.effective_input_binding(&input, Some(json!(0))),
        EffectiveInputBinding::Literal(json!(42))
    );
    document.set_literal(input.clone(), None).unwrap();
    assert_eq!(
        document.effective_input_binding(&input, Some(json!(0))),
        EffectiveInputBinding::ProtocolDefault(json!(0))
    );
}

#[test]
fn btree_maps_produce_stable_serialization() {
    let first = node_id(1);
    let second = node_id(2);
    let mut forward = GraphDocument::default();
    forward.create_node(node(first)).unwrap();
    forward.create_node(node(second)).unwrap();
    forward
        .set_literal(declared(second, "input"), Some(json!(42)))
        .unwrap();

    let mut reverse = GraphDocument::default();
    reverse.create_node(node(second)).unwrap();
    reverse.create_node(node(first)).unwrap();
    reverse
        .set_literal(declared(second, "input"), Some(json!(42)))
        .unwrap();

    let serialized = serde_json::to_string(&forward).unwrap();
    assert_eq!(serialized, serde_json::to_string(&reverse).unwrap());
    let restored: GraphDocument = serde_json::from_str(&serialized).unwrap();
    assert_eq!(restored.nodes, forward.nodes);
    assert_eq!(restored.input_states, forward.input_states);
}

#[test]
fn create_and_connect_transaction_undoes_and_redoes_with_original_identities() {
    let path = graph_path("events/history-test");
    let first = node_id(101);
    let second = node_id(102);
    let port_instance = instance_id(103);
    let connection_id = connection_id(104);
    let dynamic_input =
        PortAddress::instance(second, PortKey::new("fields").unwrap(), port_instance);
    let connection = DocumentConnection {
        id: connection_id,
        output: declared(first, "output"),
        input: dynamic_input.clone(),
        order: None,
    };
    let patch = GraphDocumentPatch::new(vec![
        GraphDocumentOperation::InsertNode { node: node(first) },
        GraphDocumentOperation::InsertNode { node: node(second) },
        GraphDocumentOperation::InsertPortBinding {
            address: dynamic_input.clone(),
            binding: binding(),
        },
        GraphDocumentOperation::InsertConnection {
            connection: connection.clone(),
        },
    ]);
    let transaction = ProjectHistoryTransaction::graph(
        operation_id(105),
        path.clone(),
        GraphRevision::INITIAL,
        patch,
    );
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([(path.clone(), GraphDocument::default())]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let mut history = ProjectHistory::default();
    let original = state.graphs.get(&path).unwrap().clone();

    history.apply_transaction(&mut state, transaction).unwrap();
    let applied = state.graphs.get(&path).unwrap();
    assert!(applied.nodes.contains_key(&first));
    assert_eq!(applied.connections.get(&connection_id), Some(&connection));
    assert!(applied.port_bindings.contains_key(&dynamic_input));
    assert_eq!(applied.revision.get(), 1);
    assert_eq!(state.revision.get(), 1);
    let applied = applied.clone();

    history.undo(&mut state).unwrap();
    let undone = state.graphs.get(&path).unwrap();
    assert!(undone.nodes.is_empty());
    assert!(undone.connections.is_empty());
    assert!(undone.port_bindings.is_empty());
    assert_graph_content_eq(undone, &original);
    assert_eq!(undone.revision.get(), 2);
    assert_eq!(state.revision.get(), 2);

    history.redo(&mut state).unwrap();
    let redone = state.graphs.get(&path).unwrap();
    assert_eq!(redone.nodes.get(&first).unwrap().id, first);
    assert_eq!(redone.nodes.get(&second).unwrap().id, second);
    assert_eq!(redone.connections.get(&connection_id), Some(&connection));
    assert!(redone.port_bindings.contains_key(&PortAddress::instance(
        second,
        PortKey::new("fields").unwrap(),
        port_instance,
    )));
    assert_graph_content_eq(redone, &applied);
    assert_eq!(redone.revision.get(), 3);
    assert_eq!(state.revision.get(), 3);
}

#[test]
fn failed_multi_resource_transaction_is_atomic() {
    let first_path = graph_path("events/first");
    let second_path = graph_path("events/second");
    let valid_node = node(node_id(201));
    let missing_node = node_id(202);
    let first_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: valid_node,
    }]);
    let invalid_connection = DocumentConnection {
        id: connection_id(203),
        output: declared(missing_node, "output"),
        input: declared(missing_node, "input"),
        order: None,
    };
    let second_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertConnection {
        connection: invalid_connection,
    }]);
    let transaction = ProjectHistoryTransaction::new(
        operation_id(204),
        vec![
            ResourcePatch::graph(first_path.clone(), GraphRevision::INITIAL, first_patch),
            ResourcePatch::graph(second_path.clone(), GraphRevision::INITIAL, second_patch),
        ],
    );
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([
            (first_path, GraphDocument::default()),
            (second_path, GraphDocument::default()),
        ]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let before = state.clone();
    let mut history = ProjectHistory::default();

    assert!(matches!(
        history.apply_transaction(&mut state, transaction),
        Err(HistoryError::Patch { .. })
    ));
    assert_eq!(state, before);
    assert_eq!(history.undo_len(), 0);
    assert_eq!(history.redo_len(), 0);
}

#[test]
fn normal_mutation_after_undo_clears_redo_branch() {
    let path = graph_path("events/branch");
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([(path.clone(), GraphDocument::default())]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let mut history = ProjectHistory::default();
    let first_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: node(node_id(301)),
    }]);
    history
        .apply_transaction(
            &mut state,
            ProjectHistoryTransaction::graph(
                operation_id(302),
                path.clone(),
                GraphRevision::INITIAL,
                first_patch,
            ),
        )
        .unwrap();
    history.undo(&mut state).unwrap();
    assert!(history.can_redo());

    let branch_revision = state.graphs.get(&path).unwrap().revision;
    let branch_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: node(node_id(303)),
    }]);
    history
        .apply_transaction(
            &mut state,
            ProjectHistoryTransaction::graph(
                operation_id(304),
                path,
                branch_revision,
                branch_patch,
            ),
        )
        .unwrap();

    assert!(!history.can_redo());
    assert!(matches!(
        history.redo(&mut state),
        Err(HistoryError::NothingToRedo)
    ));
    assert_eq!(state.revision.get(), 3);
}

#[test]
fn graph_patch_updates_node_content_reversibly() {
    let id = node_id(350);
    let before = node(id);
    let mut after = before.clone();
    after.position = NodePosition { x: 8.0, y: 13.0 };
    after.user_label = Some("updated".to_owned());
    after.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        json!(42),
    );
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::UpdateNode {
        before: before.clone(),
        after: after.clone(),
    }]);
    let mut document = GraphDocument::default();
    document.create_node(before.clone()).unwrap();

    document.apply_patch(&patch).unwrap();
    assert_eq!(document.nodes.get(&id), Some(&after));

    document.apply_patch(&patch.inverse()).unwrap();
    assert_eq!(document.nodes.get(&id), Some(&before));
    assert_eq!(document.revision.get(), 3);
}

#[test]
fn patch_kind_mismatch_is_rejected_without_mutation() {
    let path = graph_path("events/kind-mismatch");

    let function_patch = FunctionDocumentPatch::default();
    let resource_patch = ResourcePatch {
        resource: ResourceKey::Graph(path.clone()),
        before_revision: GraphRevision::INITIAL,
        after_revision: GraphRevision::new(1),
        forward: ResourceDocumentPatch::Function(function_patch.clone()),
        inverse: ResourceDocumentPatch::Function(function_patch.inverse()),
    };
    let transaction = ProjectHistoryTransaction::new(operation_id(361), vec![resource_patch]);
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([(path, GraphDocument::default())]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let before = state.clone();
    let mut history = ProjectHistory::default();

    assert!(matches!(
        history.apply_transaction(&mut state, transaction),
        Err(HistoryError::ResourceKindMismatch {
            patch_kind: ResourceKind::Function,
            ..
        })
    ));
    assert_eq!(state, before);
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn graph_patch_failure_leaves_document_and_revision_unchanged() {
    let existing = node(node_id(401));
    let mut document = GraphDocument::default();
    document.create_node(existing.clone()).unwrap();
    let before = document.clone();
    let patch = GraphDocumentPatch::new(vec![
        GraphDocumentOperation::InsertNode {
            node: node(node_id(402)),
        },
        GraphDocumentOperation::InsertNode { node: existing },
    ]);

    assert!(matches!(
        document.apply_patch(&patch),
        Err(DocumentError::DuplicateNode(_))
    ));
    assert_eq!(document, before);
}

#[test]
fn mutation_rejects_wrong_resource_without_changing_the_graph() {
    let path = graph_path("events/main");
    let requested = ResourceKey::Graph(graph_path("events/other"));
    let mut store = RevisionedGraphStore::new(path.clone(), GraphDocument::default());
    let before = store.document().clone();

    let result = store.apply_mutation(MutationRequest::new(
        requested.clone(),
        ResourceRevision::INITIAL,
        operation_id(500),
        GraphMutation::CreateNode {
            node: node(node_id(501)),
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::ResourceMismatch { requested: actual, store: expected })
            if actual == requested && expected == ResourceKey::Graph(path)
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn mutation_rejects_stale_revision_without_changing_the_graph() {
    let path = graph_path("events/main");
    let resource = ResourceKey::Graph(path.clone());
    let mut store = RevisionedGraphStore::new(path, GraphDocument::default());
    store
        .apply_mutation(MutationRequest::new(
            resource.clone(),
            ResourceRevision::INITIAL,
            operation_id(502),
            GraphMutation::CreateNode {
                node: node(node_id(503)),
            },
        ))
        .unwrap();
    let before = store.document().clone();

    let result = store.apply_mutation(MutationRequest::new(
        resource,
        ResourceRevision::INITIAL,
        operation_id(504),
        GraphMutation::CreateNode {
            node: node(node_id(505)),
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::StaleRevision {
            base_revision,
            current_revision,
        }) if base_revision == ResourceRevision::INITIAL
            && current_revision == ResourceRevision::new(1)
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn mutation_events_use_the_complete_graph_envelope() {
    let path = graph_path("events/main");
    let resource = ResourceKey::Graph(path.clone());
    let operation = operation_id(510);
    let mut store = RevisionedGraphStore::new(path.clone(), GraphDocument::default());

    let event = store
        .apply_mutation(MutationRequest::new(
            resource,
            ResourceRevision::INITIAL,
            operation,
            GraphMutation::CreateNode {
                node: node(node_id(511)),
            },
        ))
        .unwrap();

    assert_eq!(event.graph_path, path);
    assert_eq!(event.from_revision, ResourceRevision::INITIAL);
    assert_eq!(event.to_revision, ResourceRevision::new(1));
    assert_eq!(event.caused_by, Some(operation));
    assert_eq!(event.payload.operations.len(), 1);
}

#[test]
fn revision_gap_reports_the_missing_delta_range() {
    let event = GraphDeltaEvent {
        graph_path: graph_path("events/main"),
        from_revision: ResourceRevision::new(4),
        to_revision: ResourceRevision::new(5),
        caused_by: None,
        payload: GraphDocumentPatch::new(Vec::new()),
    };

    assert_eq!(
        detect_revision_gap(ResourceRevision::new(2), &event),
        Some(RevisionGap {
            expected_before_revision: ResourceRevision::new(2),
            actual_before_revision: ResourceRevision::new(4),
        })
    );
}

fn compilation_basis(path: &str, revision: GraphRevision) -> CompilationBasisToken {
    CompilationBasisToken::new(
        graph_path(path),
        revision,
        CompilationRegistryFingerprint::from_bytes([7; 32]),
        BTreeMap::from([(
            CompilationResourceKey::new("schema/source"),
            CompilationResourceVersion::new("v1"),
        )]),
    )
}

fn projected_member(path: &str, revision: GraphRevision, node_id: NodeId) -> ProjectedMemberRef {
    ProjectedMemberRef::new(
        compilation_basis(path, revision),
        node_id,
        PortKey::new("fields").unwrap(),
        DynamicMemberLocator::SchemaField {
            source: SchemaSourceIdentity("source".into()),
            field: SchemaFieldIdentity("field".into()),
        },
    )
}

fn authorization(member: ProjectedMemberRef) -> MaterializationAuthorization {
    MaterializationAuthorization::new(member, OrderKey("a".into()))
}

#[test]
fn compilation_basis_token_preserves_the_complete_resolver_basis() {
    let basis = compilation_basis("events/main", GraphRevision::new(3));
    let resource = CompilationResourceKey::new("schema/source");

    assert_eq!(basis.graph_path(), &graph_path("events/main"));
    assert_eq!(basis.graph_revision(), GraphRevision::new(3));
    assert_eq!(basis.registry_fingerprint().as_bytes(), &[7; 32]);
    assert_eq!(
        basis
            .resource_versions()
            .get(&resource)
            .map(|value| value.as_str()),
        Some("v1")
    );
}

#[test]
fn projected_member_rejects_a_stale_compilation_basis() {
    let path = graph_path("events/main");
    let source = node_id(530);
    let target = node_id(531);
    let mut document = GraphDocument::default();
    document.create_node(node(source)).unwrap();
    document.create_node(node(target)).unwrap();
    let mut store = RevisionedGraphStore::new(path.clone(), document);
    let before = store.document().clone();
    let member = projected_member("events/main", ResourceRevision::INITIAL, target);
    let authorization = authorization(member.clone());

    let result = store.apply_mutation(MutationRequest::new(
        ResourceKey::Graph(path),
        store.revision(),
        operation_id(532),
        GraphMutation::MaterializeProjectedMemberAndConnect {
            member,
            authorization,
            output: declared(source, "output"),
            order: None,
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::CompilationBasisStale { .. })
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn projected_member_rejects_authorization_for_another_member() {
    let path = graph_path("events/main");
    let source = node_id(535);
    let target = node_id(536);
    let mut document = GraphDocument::default();
    document.create_node(node(source)).unwrap();
    document.create_node(node(target)).unwrap();
    let mut store = RevisionedGraphStore::new(path.clone(), document);
    let member = projected_member("events/main", store.revision(), target);
    let other = ProjectedMemberRef::new(
        member.basis().clone(),
        target,
        member.template().clone(),
        DynamicMemberLocator::SchemaField {
            source: SchemaSourceIdentity("source".into()),
            field: SchemaFieldIdentity("forged".into()),
        },
    );
    let before = store.document().clone();

    let result = store.apply_mutation(MutationRequest::new(
        ResourceKey::Graph(path),
        store.revision(),
        operation_id(537),
        GraphMutation::MaterializeProjectedMemberAndConnect {
            member,
            authorization: authorization(other),
            output: declared(source, "output"),
            order: None,
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::MaterializationUnauthorized)
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn projected_member_materialization_and_connection_commit_atomically() {
    let path = graph_path("events/main");
    let resource = ResourceKey::Graph(path.clone());
    let source = node_id(540);
    let target = node_id(541);
    let mut document = GraphDocument::default();
    document.create_node(node(source)).unwrap();
    document.create_node(node(target)).unwrap();
    let mut store = RevisionedGraphStore::new(path, document);
    let before_revision = store.revision();
    let member = projected_member("events/main", before_revision, target);

    let event = store
        .apply_mutation(MutationRequest::new(
            resource.clone(),
            before_revision,
            operation_id(542),
            GraphMutation::MaterializeProjectedMemberAndConnect {
                authorization: authorization(member.clone()),
                member,
                output: declared(source, "output"),
                order: None,
            },
        ))
        .unwrap();

    assert_eq!(event.to_revision, before_revision.next());
    assert_eq!(store.document().port_bindings.len(), 1);
    assert_eq!(store.document().connections.len(), 1);
    let address = store.document().port_bindings.keys().next().unwrap();
    assert!(matches!(address.port, PortRef::Instance { .. }));
    assert_eq!(
        store.document().connections.values().next().unwrap().input,
        address.clone()
    );

    let invalid_source = node_id(543);
    let before_failed_request = store.document().clone();
    let member = projected_member("events/main", store.revision(), target);
    let result = store.apply_mutation(MutationRequest::new(
        resource,
        store.revision(),
        operation_id(544),
        GraphMutation::MaterializeProjectedMemberAndConnect {
            authorization: authorization(member.clone()),
            member,
            output: declared(invalid_source, "output"),
            order: None,
        },
    ));

    assert!(matches!(result, Err(MutationConflict::Document(_))));
    assert_eq!(store.document(), &before_failed_request);
}

fn function_key(value: &str) -> FunctionResourceKey {
    FunctionResourceKey(value.into())
}

fn variable_key(value: &str) -> VariableResourceKey {
    VariableResourceKey(value.into())
}

fn signature(parameter_name: &str) -> FunctionSignature {
    FunctionSignature {
        parameters: vec![FunctionParameter {
            id: FunctionParameterId("parameter-1".into()),
            name: parameter_name.into(),
            type_name: "number".into(),
        }],
        return_type: Some("number".into()),
    }
}

#[test]
fn function_signature_and_caller_graph_undo_as_one_project_transaction() {
    let graph_path = graph_path("events/caller");
    let function_key = function_key("functions/callee");
    let caller_node = node_id(620);
    let before_signature = signature("old");
    let after_signature = signature("new");
    let graph_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: node(caller_node),
    }]);
    let function_patch =
        FunctionDocumentPatch::new(before_signature.clone(), after_signature.clone());
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([(graph_path.clone(), GraphDocument::default())]),
        BTreeMap::from([(
            function_key.clone(),
            FunctionDocument::new(before_signature.clone()),
        )]),
        BTreeMap::new(),
    );
    let transaction = ProjectHistoryTransaction::new(
        operation_id(621),
        vec![
            ResourcePatch::function(
                function_key.clone(),
                ResourceRevision::INITIAL,
                function_patch,
            ),
            ResourcePatch::graph(graph_path.clone(), ResourceRevision::INITIAL, graph_patch),
        ],
    );
    let mut history = ProjectHistory::default();

    history.apply_transaction(&mut state, transaction).unwrap();
    assert_eq!(state.functions[&function_key].signature, after_signature);
    assert!(state.graphs[&graph_path].nodes.contains_key(&caller_node));

    history.undo(&mut state).unwrap();
    assert_eq!(state.functions[&function_key].signature, before_signature);
    assert!(state.graphs[&graph_path].nodes.is_empty());
    assert_eq!(state.functions[&function_key].revision.get(), 2);
    assert_eq!(state.graphs[&graph_path].revision.get(), 2);
    assert_eq!(state.revision.get(), 2);
}

#[test]
fn variable_patch_is_reversible_and_monotonic() {
    let key = variable_key("variables/threshold");
    let patch = VariableDocumentPatch::new(json!(10), json!(20));
    let mut state = ProjectDocumentState::new(
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::from([(key.clone(), VariableDocument::new(json!(10)))]),
    );
    let transaction = ProjectHistoryTransaction::new(
        operation_id(630),
        vec![ResourcePatch::variable(
            key.clone(),
            ResourceRevision::INITIAL,
            patch,
        )],
    );
    let mut history = ProjectHistory::default();

    history.apply_transaction(&mut state, transaction).unwrap();
    assert_eq!(state.variables[&key].value, json!(20));
    history.undo(&mut state).unwrap();
    assert_eq!(state.variables[&key].value, json!(10));
    assert_eq!(state.variables[&key].revision.get(), 2);
}

#[test]
fn reload_replaces_project_state_and_clears_history() {
    let path = graph_path("events/reload");
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([(path.clone(), GraphDocument::default())]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let mut history = ProjectHistory::default();
    history
        .apply_transaction(
            &mut state,
            ProjectHistoryTransaction::graph(
                operation_id(640),
                path,
                ResourceRevision::INITIAL,
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node(node_id(641)),
                }]),
            ),
        )
        .unwrap();

    let replacement = ProjectDocumentState::default();
    history.reload(&mut state, replacement.clone());

    assert_eq!(state, replacement);
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}
