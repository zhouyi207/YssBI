use super::*;

#[test]
fn editor_mutation_returns_correlated_delta_projection_and_history_status() {
    let state = state_with_empty_graph();
    let operation_id = OperationId::new();
    let request = editor_mutation_request(GraphRevision::INITIAL, operation_id);

    let result = state
        .apply_editor_graph_mutation(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path(),
            "en-US",
            request,
        )
        .unwrap();

    assert_eq!(result.delta.caused_by, Some(operation_id));
    assert_eq!(result.delta.from_revision, GraphRevision::INITIAL);
    assert_eq!(
        result.delta.to_revision.get(),
        result.projection_replacement.projection.source_revision
    );
    assert!(result.history.can_undo);
    assert!(!result.history.can_redo);
}

#[test]
fn dynamic_merge_input_create_and_connect_serializes_parseable_internal_failure() {
    use crate::node_system::document::{
        DynamicPortBinding, GraphResourcePath as DocumentGraphResourcePath, OrderKey,
        PortInstanceId,
    };

    let path = graph_path();
    let begin = node("yssbi.project.event.begin");
    let merge = node("yssbi.control.merge");
    let connected_instance = PortInstanceId::from_uuid(uuid::Uuid::from_u128(2));
    let unconnected_instance = PortInstanceId::from_uuid(uuid::Uuid::from_u128(1));
    let connected_enter =
        PortAddress::instance(merge.id, PortKey::new("enter").unwrap(), connected_instance);
    let unconnected_enter = PortAddress::instance(
        merge.id,
        PortKey::new("enter").unwrap(),
        unconnected_instance,
    );
    let connection_id = ConnectionId::new();
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    graph.document.revision = GraphRevision::new(1);
    graph.document.nodes.insert(begin.id, begin.clone());
    graph.document.nodes.insert(merge.id, merge.clone());
    graph.document.port_bindings.insert(
        connected_enter.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey("00000".into()),
        },
    );
    graph.document.port_bindings.insert(
        unconnected_enter.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey("00001".into()),
        },
    );
    graph.document.connections.insert(
        connection_id,
        DocumentConnection {
            id: connection_id,
            output: PortAddress::declared(begin.id, PortKey::new("then").unwrap()),
            input: connected_enter,
            order: None,
        },
    );
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "dynamic-merge-create-connect",
        ProjectData::new(),
    ));
    state.insert_graph(path.clone(), graph).unwrap();

    let result = state
        .apply_editor_graph_mutation(
            &current_project_instance_id(&state),
            &path,
            "zh-CN",
            MutationRequest::new(
                ResourceKey::Graph(DocumentGraphResourcePath(path.as_str().into())),
                GraphRevision::new(1),
                OperationId::new(),
                EditorGraphMutationDto::CreateNode {
                    descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
                        node_type_id: NodeTypeId::new("yssbi.control.do").unwrap(),
                    },
                    position: crate::node_system::document::NodePosition { x: 20.0, y: 30.0 },
                    user_label: None,
                    connect_from: Some(unconnected_enter.into()),
                },
            ),
        )
        .unwrap();

    let outcome = serde_json::to_value(&result.projection_replacement.projection.outcome).unwrap();
    assert_eq!(outcome["type"], "internalFailure");
    assert!(outcome.get("nodeId").is_some());
    assert!(outcome.get("node_id").is_none());
    assert_eq!(result.delta.to_revision, GraphRevision::new(2));

    let merge_projection = result
        .projection_replacement
        .projection
        .nodes
        .iter()
        .find(|node| node.node_id.as_ref() == merge.id.to_string())
        .unwrap();
    let enter_ids = merge_projection
        .ports
        .iter()
        .filter_map(|port| match &port.address {
            crate::node_system::document::PortAddressDto::Instance {
                template_key,
                instance_id,
                ..
            } if template_key.as_ref() == "enter" => Some(instance_id.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        enter_ids,
        vec![
            connected_instance.to_string(),
            unconnected_instance.to_string()
        ]
    );
}

#[test]
fn stale_editor_mutation_rejects_without_consuming_history() {
    let state = state_with_empty_graph();
    state
        .apply_editor_graph_mutation(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
        .unwrap();
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
    assert_eq!(
        state.history_status(),
        crate::node_system::document::HistoryStatusDto {
            can_undo: false,
            can_redo: true,
        }
    );

    let error = state
        .apply_editor_graph_mutation(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
        .unwrap_err();

    assert!(matches!(error, MutationConflict::StaleRevision { .. }));
    assert_eq!(
        state.history_status(),
        crate::node_system::document::HistoryStatusDto {
            can_undo: false,
            can_redo: true,
        }
    );
}

#[test]
fn undo_redo_return_atomic_replacements_and_current_history_status() {
    let state = state_with_empty_graph();
    state
        .apply_editor_graph_mutation(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
        .unwrap();

    let undo = state
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
    assert_eq!(undo.deltas[0].to_revision, GraphRevision::new(2));
    assert_eq!(undo.projection_replacements.len(), 1);
    assert_eq!(
        undo.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![graph_path().as_str().to_string()],
        }
    );
    assert_eq!(
        undo.projection_replacements[0].projection.source_revision,
        2
    );
    assert_eq!(
        undo.history,
        crate::node_system::document::HistoryStatusDto {
            can_undo: false,
            can_redo: true,
        }
    );

    let redo = state
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
    assert_eq!(redo.deltas[0].to_revision, GraphRevision::new(3));
    assert_eq!(redo.projection_replacements.len(), 1);
    assert_eq!(
        redo.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![graph_path().as_str().to_string()],
        }
    );
    assert_eq!(
        redo.projection_replacements[0].projection.source_revision,
        3
    );
    assert_eq!(
        redo.history,
        crate::node_system::document::HistoryStatusDto {
            can_undo: true,
            can_redo: false,
        }
    );
}
