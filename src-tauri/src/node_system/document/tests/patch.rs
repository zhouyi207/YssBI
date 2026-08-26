use super::*;

#[test]
fn graph_patch_revision_exhaustion_has_zero_effects() {
    let mut document = GraphDocument {
        revision: GraphRevision::new(u64::MAX),
        ..GraphDocument::default()
    };
    let before = document.clone();

    assert_eq!(
        document.apply_patch(&GraphDocumentPatch::new(Vec::new())),
        Err(DocumentError::RevisionExhausted { retained: u64::MAX })
    );
    assert_eq!(document, before);
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
    let path = graph_path("events/kind-mismatch.yssbi-event");

    let function_patch = FunctionDocumentPatch::default();
    let resource_patch = ResourcePatch {
        resource: ResourceKey::Graph(path.clone()),
        before_revision: ResourceRevision::INITIAL,
        after_revision: ResourceRevision::new(1),
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
