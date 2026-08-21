use super::*;

#[derive(Clone, Copy)]
enum Task4SubgraphMutation {
    Duplicate,
    Insert,
}

impl Task4SubgraphMutation {
    const ALL: [Self; 2] = [Self::Duplicate, Self::Insert];

    const fn label(self) -> &'static str {
        match self {
            Self::Duplicate => "duplicate",
            Self::Insert => "insert",
        }
    }
}

fn subgraph_mutation_fixture() -> (ActivatedProjectState, Vec<NodeId>) {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "task4-subgraph-mutation",
        ProjectData::new(),
    ));
    let first = NodeId::from_uuid(uuid::Uuid::from_u128(0x4_001));
    let second = NodeId::from_uuid(uuid::Uuid::from_u128(0x4_002));
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    for (id, position, label) in [
        (first, NodePosition { x: 10.0, y: 20.0 }, "First"),
        (second, NodePosition { x: 80.0, y: 90.0 }, "Second"),
    ] {
        graph.document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: NodeTypeId::new("yssbi.numeric.series.add").unwrap(),
                position,
                parameters: ParameterValues::new(),
                user_label: Some(label.into()),
            },
        );
    }
    let operand = PortKey::new("operands").unwrap();
    let instances = [
        (
            first,
            PortInstanceId::from_uuid(uuid::Uuid::from_u128(0x4_101)),
        ),
        (
            first,
            PortInstanceId::from_uuid(uuid::Uuid::from_u128(0x4_102)),
        ),
        (
            second,
            PortInstanceId::from_uuid(uuid::Uuid::from_u128(0x4_103)),
        ),
        (
            second,
            PortInstanceId::from_uuid(uuid::Uuid::from_u128(0x4_104)),
        ),
    ];
    for (index, (node_id, instance_id)) in instances.into_iter().enumerate() {
        let address = PortAddress::instance(node_id, operand.clone(), instance_id);
        graph.document.port_bindings.insert(
            address.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey(format!("operand-{index}").into()),
            },
        );
        if index != 2 {
            graph.document.input_states.insert(
                address,
                InputState {
                    literal_override: Some(
                        serde_json::to_value(crate::node_system::protocol::TypedValue {
                            value_type: crate::node_system::protocol::TypeExpr::Concrete(
                                crate::node_system::protocol::TypeId::new("core.int64").unwrap(),
                            ),
                            value: crate::node_system::protocol::Value::Integer(index as i64 + 1),
                        })
                        .unwrap(),
                    ),
                },
            );
        }
    }
    let connection_id = ConnectionId::from_uuid(uuid::Uuid::from_u128(0x4_201));
    graph.document.connections.insert(
        connection_id,
        DocumentConnection {
            id: connection_id,
            output: PortAddress::declared(first, PortKey::new("result").unwrap()),
            input: PortAddress::instance(second, operand, instances[2].1),
            order: None,
        },
    );
    state.insert_graph(graph_path(), graph).unwrap();
    (state, vec![first, second])
}

fn task4_insert_snapshot_json() -> String {
    serde_json::to_string(&ClipboardSubgraphDto {
        schema_version: 1,
        nodes: vec![ClipboardNodeDto {
            local_id: ClipboardNodeId("node/0".into()),
            creation: ClipboardNodeCreationDto::Static {
                node_type_id: NodeTypeId::new("yssbi.constant.int64").unwrap(),
            },
            parameters: ParameterValues::new(),
            user_label: Some("Inserted".into()),
            relative_position: NodePosition { x: 0.0, y: 0.0 },
        }],
        port_bindings: Vec::new(),
        input_states: Vec::new(),
        connections: Vec::new(),
    })
    .unwrap()
}

fn task4_subgraph_request(
    kind: Task4SubgraphMutation,
    sources: Vec<NodeId>,
    base_revision: GraphRevision,
    operation_id: OperationId,
) -> MutationRequest<EditorGraphMutationDto> {
    let payload = match kind {
        Task4SubgraphMutation::Duplicate => EditorGraphMutationDto::DuplicateSubgraph {
            node_ids: sources,
            offset: NodePosition { x: 40.0, y: 50.0 },
        },
        Task4SubgraphMutation::Insert => EditorGraphMutationDto::InsertSubgraph {
            snapshot_json: task4_insert_snapshot_json(),
            anchor: NodePosition { x: 50.0, y: 70.0 },
        },
    };
    MutationRequest::new(
        ResourceKey::Graph(document_path()),
        base_revision,
        operation_id,
        payload,
    )
}

fn task4_graph_document(state: &ProjectState) -> crate::node_system::document::GraphDocument {
    state.get_data().unwrap().graphs[&graph_path()]
        .document
        .clone()
}

fn assert_task4_graph_content_eq(
    left: &crate::node_system::document::GraphDocument,
    right: &crate::node_system::document::GraphDocument,
) {
    assert_eq!(left.nodes, right.nodes);
    assert_eq!(left.port_bindings, right.port_bindings);
    assert_eq!(left.input_states, right.input_states);
    assert_eq!(left.connections, right.connections);
}

#[test]
fn subgraph_mutation_advances_one_revision_and_one_history_entry() {
    for kind in Task4SubgraphMutation::ALL {
        let (state, source) = subgraph_mutation_fixture();
        let before_history = state.history_lengths_for_test();
        let operation_id = OperationId::new();
        let result = state
            .apply_editor_graph_mutation(
                &current_project_instance_id(&state),
                &graph_path(),
                "en-US",
                task4_subgraph_request(kind, source, GraphRevision::INITIAL, operation_id),
            )
            .unwrap_or_else(|error| panic!("{} mutation failed: {error}", kind.label()));

        assert_eq!(result.delta.from_revision, GraphRevision::INITIAL);
        assert_eq!(result.delta.to_revision, GraphRevision::new(1));
        assert_eq!(result.delta.caused_by, Some(operation_id));
        assert_eq!(task4_graph_document(&state).revision, GraphRevision::new(1));
        assert_eq!(
            state.history_lengths_for_test(),
            (before_history.0 + 1, before_history.1)
        );
    }
}

#[test]
fn subgraph_mutation_undoes_and_redoes_in_one_step() {
    for kind in Task4SubgraphMutation::ALL {
        let (state, source) = subgraph_mutation_fixture();
        let original = task4_graph_document(&state);
        state
            .apply_editor_graph_mutation(
                &current_project_instance_id(&state),
                &graph_path(),
                "en-US",
                task4_subgraph_request(kind, source, GraphRevision::INITIAL, OperationId::new()),
            )
            .unwrap();
        let committed = task4_graph_document(&state);
        assert_eq!(state.history_lengths_for_test(), (1, 0));

        state
            .undo_last_transaction_observed(
                &current_project_instance_id(&state),
                "en-US",
                MutationRequest::new(
                    ResourceKey::Graph(document_path()),
                    GraphRevision::new(1),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |_| {},
            )
            .unwrap();
        assert_task4_graph_content_eq(&task4_graph_document(&state), &original);
        assert_eq!(state.history_lengths_for_test(), (0, 1));

        state
            .redo_last_transaction_observed(
                &current_project_instance_id(&state),
                "en-US",
                MutationRequest::new(
                    ResourceKey::Graph(document_path()),
                    GraphRevision::new(2),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |_| {},
            )
            .unwrap();
        assert_task4_graph_content_eq(&task4_graph_document(&state), &committed);
        assert_eq!(state.history_lengths_for_test(), (1, 0));
    }
}

#[test]
fn subgraph_mutation_returns_complete_delta_and_projection() {
    for kind in Task4SubgraphMutation::ALL {
        let (state, source) = subgraph_mutation_fixture();
        let before = task4_graph_document(&state);
        let result = state
            .apply_editor_graph_mutation(
                &current_project_instance_id(&state),
                &graph_path(),
                "en-US",
                task4_subgraph_request(kind, source, GraphRevision::INITIAL, OperationId::new()),
            )
            .unwrap();
        let committed = task4_graph_document(&state);
        let mut reconstructed = before;
        reconstructed.apply_patch(&result.delta.payload).unwrap();
        assert_eq!(reconstructed, committed);
        assert_eq!(
            result.projection_replacement.projection,
            state.graph_projection(&graph_path(), "en-US").unwrap()
        );
        assert_eq!(
            result.projection_replacement.projection.source_revision,
            result.delta.to_revision.get()
        );
    }
}

#[test]
fn subgraph_mutation_stale_revision_has_zero_effects() {
    for kind in Task4SubgraphMutation::ALL {
        let (state, source) = subgraph_mutation_fixture();
        let before = task4_graph_document(&state);
        let history_before = state.history_lengths_for_test();
        let generation_before = state.authority_generation_for_test();
        let error = state
            .apply_editor_graph_mutation(
                &current_project_instance_id(&state),
                &graph_path(),
                "en-US",
                task4_subgraph_request(kind, source, GraphRevision::new(9), OperationId::new()),
            )
            .unwrap_err();

        assert!(matches!(error, MutationConflict::StaleRevision { .. }));
        assert_eq!(task4_graph_document(&state), before);
        assert_eq!(state.history_lengths_for_test(), history_before);
        assert_eq!(state.authority_generation_for_test(), generation_before);
    }
}

#[test]
fn subgraph_mutation_same_revision_allows_exactly_one_commit() {
    for kind in Task4SubgraphMutation::ALL {
        let (state, source) = subgraph_mutation_fixture();
        let planning_complete = std::sync::Arc::new(std::sync::Barrier::new(2));
        state.set_mutation_publication_test_hook({
            let planning_complete = planning_complete.clone();
            std::sync::Arc::new(move || {
                planning_complete.wait();
            })
        });
        let outcomes = std::thread::scope(|scope| {
            let handles = [OperationId::new(), OperationId::new()].map(|operation_id| {
                let state = &state;
                let source = source.clone();
                scope.spawn(move || {
                    state.apply_editor_graph_mutation(
                        &current_project_instance_id(state),
                        &graph_path(),
                        "en-US",
                        task4_subgraph_request(kind, source, GraphRevision::INITIAL, operation_id),
                    )
                })
            });
            handles.map(|handle| handle.join().unwrap())
        });

        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| {
                    matches!(
                        outcome,
                        Err(MutationConflict::StaleRevision { .. })
                            | Err(MutationConflict::CatalogResourceStale(_))
                    )
                })
                .count(),
            1
        );
        assert_eq!(task4_graph_document(&state).revision, GraphRevision::new(1));
        assert_eq!(state.history_lengths_for_test(), (1, 0));
    }
}

#[test]
fn subgraph_mutation_authority_change_after_catalog_snapshot_has_zero_effects() {
    for kind in Task4SubgraphMutation::ALL {
        let (state, source) = subgraph_mutation_fixture();
        let before = task4_graph_document(&state);
        let history_before = state.history_lengths_for_test();
        let generation_before = state.authority_generation_for_test();
        let hook_state = state.clone();
        state.set_catalog_mutation_before_publication_test_hook(std::sync::Arc::new(move || {
            hook_state
                .mutation_publication
                .lock()
                .unwrap()
                .advance_authority_generation();
        }));

        let error = state
            .apply_editor_graph_mutation(
                &current_project_instance_id(&state),
                &graph_path(),
                "en-US",
                task4_subgraph_request(kind, source, GraphRevision::INITIAL, OperationId::new()),
            )
            .unwrap_err();
        assert_eq!(error.code(), "catalog_resource_stale");
        assert_eq!(task4_graph_document(&state), before);
        assert_eq!(state.history_lengths_for_test(), history_before);
        assert_eq!(state.authority_generation_for_test(), generation_before + 1);
    }
}

#[test]
fn insert_subgraph_invalid_raw_snapshot_has_zero_effects() {
    let (state, _) = subgraph_mutation_fixture();
    let before = task4_graph_document(&state);
    let history_before = state.history_lengths_for_test();
    let error = state
        .apply_editor_graph_mutation(
            &current_project_instance_id(&state),
            &graph_path(),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                EditorGraphMutationDto::InsertSubgraph {
                    snapshot_json: "{".into(),
                    anchor: NodePosition { x: 0.0, y: 0.0 },
                },
            ),
        )
        .unwrap_err();
    assert_eq!(error.code(), "clipboard_subgraph_invalid");
    assert_eq!(task4_graph_document(&state), before);
    assert_eq!(state.history_lengths_for_test(), history_before);
}
