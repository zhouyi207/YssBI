use super::*;

#[test]
fn worksheet_resource_move_history_round_trips_without_graph_payload() {
    let document = worksheet_document(ResourceRevision::new(4), "database", "line");
    let mut transaction = ProjectHistoryTransaction::new(operation_id(633), Vec::new());
    transaction.persistence = HistoryPersistencePolicy::DurableResourceMove;
    transaction.resource_move = Some(ResourceMoveHistoryPatch {
        from: "worksheets/Before.yssbi-worksheet".into(),
        to: "worksheets/After.yssbi-worksheet".into(),
        kind: ResourceLifecycleKind::Worksheet,
        payload: ResourceMoveHistoryPayload::Worksheet {
            document: document.clone(),
        },
    });

    let encoded = serde_json::to_value(&transaction).unwrap();
    assert!(encoded.get("graph_resource_move").is_none());
    assert!(encoded.to_string().contains("document"));
    assert!(!encoded.to_string().contains("persisted_move_payload"));
    assert_eq!(
        serde_json::from_value::<ProjectHistoryTransaction>(encoded).unwrap(),
        transaction
    );

    let before_key = WorksheetResourceKey("worksheets/Before.yssbi-worksheet".into());
    let after_key = WorksheetResourceKey("worksheets/After.yssbi-worksheet".into());
    let mut state = ProjectDocumentState::default();
    state.worksheets.insert(after_key.clone(), document.clone());
    state
        .worksheet_revisions
        .insert(after_key.clone(), ResourceRevision::new(4));
    let mut history = ProjectHistory::default();
    history.record_committed_transaction(transaction);
    history.undo(&mut state).unwrap();
    assert!(!state.worksheets.contains_key(&after_key));
    assert_eq!(
        state.worksheets[&before_key].revision,
        ResourceRevision::new(5)
    );
    history.redo(&mut state).unwrap();
    assert!(!state.worksheets.contains_key(&before_key));
    assert_eq!(
        state.worksheets[&after_key].revision,
        ResourceRevision::new(6)
    );
}

#[test]
fn worksheet_lifecycle_history_preserves_document_for_restore() {
    let document = worksheet_document(ResourceRevision::new(7), "database", "area");
    let path = "worksheets/Recoverable.yssbi-worksheet";
    let lifecycle = ResourceLifecyclePatch {
        before: Some(ResourceLifecycleState {
            revision: document.revision,
            path: path.into(),
            kind: ResourceLifecycleKind::Worksheet,
            name: "Recoverable".into(),
        }),
        after: None,
    };
    let mut transaction = ProjectHistoryTransaction::new(operation_id(634), Vec::new());
    transaction.resource_lifecycle = Some(ResourceLifecycleHistoryPatch {
        forward: lifecycle,
        payload: ResourceLifecycleHistoryPayload::Worksheet {
            document: document.clone(),
        },
    });

    let encoded = serde_json::to_value(&transaction).unwrap();
    let restored: ProjectHistoryTransaction = serde_json::from_value(encoded).unwrap();
    assert_eq!(
        restored.resource_lifecycle,
        Some(ResourceLifecycleHistoryPatch {
            forward: ResourceLifecyclePatch {
                before: Some(ResourceLifecycleState {
                    revision: document.revision,
                    path: path.into(),
                    kind: ResourceLifecycleKind::Worksheet,
                    name: "Recoverable".into(),
                }),
                after: None,
            },
            payload: ResourceLifecycleHistoryPayload::Worksheet {
                document: document.clone(),
            },
        })
    );

    let key = WorksheetResourceKey(path.into());
    let mut state = ProjectDocumentState::default();
    state
        .worksheet_revisions
        .insert(key.clone(), ResourceRevision::new(8));
    let mut history = ProjectHistory::default();
    history.record_committed_transaction(restored);
    history.undo(&mut state).unwrap();
    assert_eq!(state.worksheets[&key].chart_type, document.chart_type);
    assert_eq!(state.worksheet_revisions[&key], ResourceRevision::new(9));
    history.redo(&mut state).unwrap();
    assert!(!state.worksheets.contains_key(&key));
    assert_eq!(state.worksheet_revisions[&key], ResourceRevision::new(10));
}

#[test]
fn history_state_tracks_worksheet_paths_and_revisions() {
    let key = WorksheetResourceKey("worksheets/Tracked.yssbi-worksheet".into());
    let before = worksheet_state("database", "histogram");
    let after = worksheet_state("database", "scatter");
    let mut state = ProjectDocumentState::default();
    state.worksheets.insert(
        key.clone(),
        worksheet_document(ResourceRevision::new(3), "database", "histogram"),
    );
    state
        .worksheet_revisions
        .insert(key.clone(), ResourceRevision::new(3));
    let transaction = ProjectHistoryTransaction::new(
        operation_id(635),
        vec![ResourcePatch::worksheet(
            key.clone(),
            ResourceRevision::new(3),
            WorksheetDocumentPatch {
                before: before.clone(),
                after: after.clone(),
            },
        )],
    );
    let mut history = ProjectHistory::default();

    history.apply_transaction(&mut state, transaction).unwrap();
    assert_eq!(state.worksheets[&key].chart_type, "scatter");
    assert_eq!(state.worksheets[&key].revision, ResourceRevision::new(4));
    assert_eq!(state.worksheet_revisions[&key], ResourceRevision::new(4));

    history.undo(&mut state).unwrap();
    assert_eq!(state.worksheets[&key].chart_type, "histogram");
    assert_eq!(state.worksheets[&key].revision, ResourceRevision::new(5));
    assert_eq!(state.worksheet_revisions[&key], ResourceRevision::new(5));
}

#[test]
fn reload_replaces_project_state_and_clears_history() {
    let path = graph_path("events/reload.yssbi-event");
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
                GraphRevision::INITIAL,
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
