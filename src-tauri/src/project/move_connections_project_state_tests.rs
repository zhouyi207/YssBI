use super::*;
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, EditorGraphMutationDto, GraphRevision,
    MutationRequest, NodeId, OperationId, ParameterValues, PortAddress, ResourceKey,
};
use crate::node_system::protocol::{NodeTypeId, PortKey};

fn node(id: NodeId, node_type: &str) -> DocumentNode {
    DocumentNode {
        id,
        node_type: NodeTypeId::new(node_type).unwrap(),
        position: crate::node_system::document::NodePosition { x: 0.0, y: 0.0 },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

#[test]
fn phase1_move_connections_project_state_snapshot_reaches_planner() {
    let project = crate::project::fixtures::TempProject::activate(
        "phase1-move-connections-project-state",
        ProjectData::new(),
    );
    let state = project.state();
    let graph_path = GraphResourcePath::new("events/MoveConnections.yssbi-event").unwrap();
    let source_node = NodeId::from_uuid(uuid::Uuid::from_u128(0x5101));
    let target_node = NodeId::from_uuid(uuid::Uuid::from_u128(0x5102));
    let empty_node = NodeId::from_uuid(uuid::Uuid::from_u128(0x5103));
    let input_node = NodeId::from_uuid(uuid::Uuid::from_u128(0x5104));
    let source = PortAddress::declared(source_node, PortKey::new("value").unwrap());
    let target = PortAddress::declared(target_node, PortKey::new("value").unwrap());
    let input = PortAddress::declared(input_node, PortKey::new("start").unwrap());
    let mut graph = GraphResourceDocument::new("MoveConnections", GraphDocumentKind::Event);
    for document_node in [
        node(source_node, "yssbi.constant.int64"),
        node(target_node, "yssbi.constant.int64"),
        node(empty_node, "yssbi.constant.int64"),
        node(input_node, "yssbi.dataframe.series.int_range"),
    ] {
        graph.document.nodes.insert(document_node.id, document_node);
    }
    let original = DocumentConnection {
        id: ConnectionId::from_uuid(uuid::Uuid::from_u128(0x5110)),
        output: source.clone(),
        input,
        order: None,
    };
    graph.document.connections.insert(original.id, original);
    state.insert_graph(graph_path.clone(), graph).unwrap();
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let resource = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        graph_path.as_str().into(),
    ));

    let result = state
        .apply_editor_graph_mutation(
            &project_instance_id,
            &graph_path,
            "en-US",
            MutationRequest::new(
                resource.clone(),
                GraphRevision::INITIAL,
                OperationId::from_uuid(uuid::Uuid::from_u128(0x5120)),
                EditorGraphMutationDto::MoveConnections {
                    source: source.into(),
                    target: target.clone().into(),
                },
            ),
        )
        .unwrap();
    assert_eq!(result.delta.payload.operations.len(), 2);

    let error = state
        .apply_editor_graph_mutation(
            &project_instance_id,
            &graph_path,
            "en-US",
            MutationRequest::new(
                resource,
                result.delta.to_revision,
                OperationId::from_uuid(uuid::Uuid::from_u128(0x5121)),
                EditorGraphMutationDto::MoveConnections {
                    source: PortAddress::declared(empty_node, PortKey::new("value").unwrap())
                        .into(),
                    target: target.into(),
                },
            ),
        )
        .unwrap_err();
    assert_eq!(error.code(), "graph_connection_move_source_empty");
}
