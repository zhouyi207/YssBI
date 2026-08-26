use super::*;

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
            ResourcePatch::graph(graph_path.clone(), GraphRevision::INITIAL, graph_patch),
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
    let patch = VariableDocumentPatch::new(Some(json!(10)), Some(json!(20)));
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
    assert_eq!(state.variables[&key].value, Some(json!(20)));
    history.undo(&mut state).unwrap();
    assert_eq!(state.variables[&key].value, Some(json!(10)));
    assert_eq!(state.variables[&key].revision.get(), 2);
}
