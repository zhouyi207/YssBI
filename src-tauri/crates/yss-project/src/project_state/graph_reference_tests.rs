use super::*;
use yss_graph_document::{
    DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    GraphDocumentOperation, GraphDocumentPatch, GraphResourceKind, InputState,
    LastKnownPortMetadata, NodePosition,
};
use yss_project_identity::OperationId;
use yss_project_model::ProjectData;

fn caller(function: &GraphResourcePath) -> GraphResourceDocument {
    let mut graph = GraphResourceDocument::new("Caller", GraphResourceKind::Event);
    let call = NodeId::new();
    let text = NodeId::new();
    for (id, kind, key) in [
        (call, "yssbi.project.function.call", "target"),
        (text, "yssbi.dataframe.rename", "to"),
    ] {
        graph.document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: [(key.parse().unwrap(), serde_json::json!(function.as_str()))].into(),
                user_label: None,
            },
        );
    }
    for (i, connected) in [(0, true), (1, false)] {
        let address =
            PortAddress::instance(call, "arguments".parse().unwrap(), PortInstanceId::new());
        graph.document.port_bindings.insert(
            address.clone(),
            DynamicPortBinding::Resolved {
                origin: DynamicMemberLocator::FunctionParameter {
                    function: function.clone(),
                    parameter: yss_graph_document::FunctionParameterId::new(format!("p{i}")),
                },
                order: yss_graph_document::OrderKey::new(i.to_string()),
                last_known: LastKnownPortMetadata::default(),
            },
        );
        if connected {
            let id = ConnectionId::new();
            graph.document.connections.insert(
                id,
                DocumentConnection {
                    id,
                    output: PortAddress::declared(text, "dataframe".parse().unwrap()),
                    input: address,
                    order: None,
                },
            );
        } else {
            graph.document.input_states.insert(
                address,
                InputState {
                    literal_override: Some(yss_node_protocol::TypedValue {
                        value_type: yss_node_protocol::TypeExpr::Concrete(
                            "core.text".parse().unwrap(),
                        ),
                        value: yss_data_contract::DataValue::String(function.as_str().into()),
                    }),
                },
            );
        }
    }
    graph
}

fn assert_references(
    document: &GraphDocument,
    target: &GraphResourcePath,
    text: &GraphResourcePath,
) {
    for node in document.nodes.values() {
        let (key, expected) = if node.node_type.as_str() == "yssbi.project.function.call" {
            ("target", target)
        } else {
            ("to", text)
        };
        assert_eq!(
            node.parameters[&key.parse().unwrap()],
            serde_json::json!(expected.as_str())
        );
    }
    for binding in document.port_bindings.values() {
        let DynamicPortBinding::Resolved {
            origin: DynamicMemberLocator::FunctionParameter { function, .. },
            ..
        } = binding
        else {
            panic!("binding changed kind")
        };
        assert_eq!(function, target);
    }
}

#[test]
fn rename_updates_unloaded_callers_and_duplicate_preserves_text_and_port_identity_rules() {
    let source = GraphResourcePath::new("functions/F.yssbi-function").unwrap();
    let target = GraphResourcePath::new("functions/G.yssbi-function").unwrap();
    let path = GraphResourcePath::new("events/Caller.yssbi-event").unwrap();
    let original = caller(&source);
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("F", GraphResourceKind::Function),
    );
    data.graphs.insert(path.clone(), original.clone());
    let fixture = crate::fixtures::TempProject::activate("rename-unloaded-reference", data);
    let state = fixture.state();
    let session = state.capture_project_session().unwrap();
    state.unload_graph_resource(&path).unwrap();
    state
        .rename_graph_resource(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            "G",
            100,
            OperationId::new(),
        )
        .unwrap();
    assert!(state.read_resident_graph(&path).unwrap().is_none());
    let saved = crate::project_io::load_project_graph_from_file(
        session.root.as_path().to_str().unwrap(),
        &path,
    )
    .unwrap();
    assert_references(&saved.document, &target, &source);
    assert_eq!(saved.document.connections, original.document.connections);
    assert_eq!(saved.document.input_states, original.document.input_states);
    assert_eq!(
        saved.document.port_bindings.keys().collect::<Vec<_>>(),
        original.document.port_bindings.keys().collect::<Vec<_>>()
    );
    assert!(!session.root.as_path().join(source.as_str()).exists());
    let duplicate = duplicate_document(&original.document, &source, &target);
    assert_references(&duplicate, &target, &source);
    assert!(
        duplicate
            .nodes
            .keys()
            .all(|id| !original.document.nodes.contains_key(id))
    );
    assert_eq!(
        duplicate.input_states.values().collect::<Vec<_>>(),
        original.document.input_states.values().collect::<Vec<_>>()
    );
    assert!(
        duplicate
            .connections
            .values()
            .all(|connection| duplicate.port_bindings.contains_key(&connection.input))
    );
}

#[test]
fn rename_remaps_saved_only_references_and_reversible_history() {
    use crate::GraphHistoryAction;
    use std::sync::Arc;
    use yss_graph_document_edit::apply_graph_document_patch;
    let source = GraphResourcePath::new("functions/F.yssbi-function").unwrap();
    let target = GraphResourcePath::new("functions/G.yssbi-function").unwrap();
    let path = GraphResourcePath::new("events/Caller.yssbi-event").unwrap();
    let original = caller(&source);
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("F", GraphResourceKind::Function),
    );
    data.graphs.insert(path.clone(), original.clone());
    let fixture = crate::fixtures::TempProject::activate("rename-history-reference", data);
    let state = fixture.state();
    let session = state.capture_project_session().unwrap();
    let snapshot = state
        .read_graph_editing(&session.instance_id, &path)
        .unwrap();
    let mut operations = Vec::new();
    operations.extend(
        original
            .document
            .input_states
            .iter()
            .map(|(address, input)| GraphDocumentOperation::SetInputState {
                address: address.clone(),
                before: Some(input.clone()),
                after: None,
            }),
    );
    operations.extend(
        original
            .document
            .connections
            .values()
            .cloned()
            .map(|connection| GraphDocumentOperation::RemoveConnection { connection }),
    );
    operations.extend(
        original
            .document
            .port_bindings
            .iter()
            .map(
                |(address, binding)| GraphDocumentOperation::RemovePortBinding {
                    address: address.clone(),
                    binding: binding.clone(),
                },
            ),
    );
    operations.extend(
        original
            .document
            .nodes
            .values()
            .filter(|node| node.node_type.as_str() == "yssbi.project.function.call")
            .cloned()
            .map(|node| GraphDocumentOperation::RemoveNode { node }),
    );
    let patch = GraphDocumentPatch::new(operations);
    let mut removed = original.document.clone();
    apply_graph_document_patch(&mut removed, &patch).unwrap();
    let capture = state
        .capture_graph_edit(
            &session.instance_id,
            &path,
            snapshot.state.version,
            OperationId::new(),
            [0; 32],
        )
        .unwrap();
    state
        .commit_graph_edit(
            capture,
            Arc::new(removed.clone()),
            GraphHistoryAction::Edit(patch),
        )
        .unwrap();
    state
        .rename_graph_resource(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            "G",
            100,
            OperationId::new(),
        )
        .unwrap();
    let current = state
        .read_graph_editing(&session.instance_id, &path)
        .unwrap();
    assert_eq!(*current.document, removed);
    assert!(current.state.dirty && current.state.can_undo);
    let undo = state
        .graph_history_patch(&session.instance_id, &path, current.state.version, false)
        .unwrap()
        .unwrap();
    let mut restored = (*current.document).clone();
    apply_graph_document_patch(&mut restored, &undo).unwrap();
    assert_references(&restored, &target, &source);
    assert_eq!(restored.connections, original.document.connections);
    assert_eq!(restored.input_states, original.document.input_states);
    let capture = state
        .capture_graph_edit(
            &session.instance_id,
            &path,
            current.state.version,
            OperationId::new(),
            [1; 32],
        )
        .unwrap();
    state
        .commit_graph_edit(
            capture,
            Arc::new(restored.clone()),
            GraphHistoryAction::Undo,
        )
        .unwrap();
    let third = GraphResourcePath::new("functions/H.yssbi-function").unwrap();
    state
        .rename_graph_resource(
            &session.instance_id,
            &target,
            ResourceRevision::INITIAL.checked_next().unwrap(),
            "H",
            200,
            OperationId::new(),
        )
        .unwrap();
    let current = state
        .read_graph_editing(&session.instance_id, &path)
        .unwrap();
    assert!(current.state.can_redo);
    assert_references(&current.document, &third, &source);
    let redo = state
        .graph_history_patch(&session.instance_id, &path, current.state.version, true)
        .unwrap()
        .unwrap();
    let mut redone = (*current.document).clone();
    apply_graph_document_patch(&mut redone, &redo).unwrap();
    assert_eq!(redone, removed);
    apply_graph_document_patch(&mut restored, &undo.inverse()).unwrap();
    assert_eq!(restored, removed);
    let saved = crate::project_io::load_project_graph_from_file(
        session.root.as_path().to_str().unwrap(),
        &path,
    )
    .unwrap();
    assert_references(&saved.document, &third, &source);
}
