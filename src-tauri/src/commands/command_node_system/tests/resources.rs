use super::*;

#[test]
fn save_command_preserves_identity_revision_operation_and_emits_once() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-save-command-contract-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        path.clone(),
        GraphResourceDocument::new("Main", GraphDocumentKind::Event),
    );
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let operation_id = crate::project::OperationId::new();
    let mut events = Vec::new();

    let result = save_project_graph_with_emitter(
        &state,
        project_instance_id.clone(),
        path,
        ResourceRevision::INITIAL,
        operation_id,
        |event| events.push(event),
    )
    .unwrap();

    assert_eq!(result.project_instance_id, project_instance_id.as_str());
    assert_eq!(result.operation_id, operation_id);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        Event::Project(EventProject::ProjectSaved { result: emitted }) if emitted == &result
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stale_save_command_emits_no_event() {
    let root =
        std::env::temp_dir().join(format!("yssbi-stale-save-command-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        path.clone(),
        GraphResourceDocument::new("Main", GraphDocumentKind::Event),
    );
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
    let stale = state.capture_project_session().unwrap().instance_id;
    state.activate_project_fixture(
        root.to_string_lossy().into_owned(),
        state.get_data().unwrap(),
    );
    let mut events = Vec::new();

    let error = save_project_graph_with_emitter(
        &state,
        stale,
        path,
        ResourceRevision::INITIAL,
        crate::project::OperationId::new(),
        |event| events.push(event),
    )
    .unwrap_err();

    assert_eq!(error.code(), "stale_project_lifecycle");
    assert!(events.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rename_command_rejects_stale_project_before_registration_io_or_event() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-stale-rename-command-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let old_path = GraphResourcePath::new("events/Old.yssbi-event").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        old_path.clone(),
        GraphResourceDocument::new("Old", GraphDocumentKind::Event),
    );
    crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
    crate::project::fixtures::write_graph(&project, root.to_string_lossy().as_ref(), &old_path)
        .unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
    std::fs::write(root.join(old_path.as_str()), b"malformed graph").unwrap();
    let mut events = Vec::new();

    let error = rename_graph_resource_with_emitter(
        &state,
        ProjectInstanceId::from_existing("stale-project-instance".into()),
        old_path.clone(),
        ResourceRevision::INITIAL,
        "New",
        1,
        OperationId::new(),
        |event| events.push(event),
    )
    .unwrap_err();

    assert_eq!(state.resource_lifecycle_entry_count(), 0);
    assert!(events.is_empty());
    assert!(root.join(old_path.as_str()).exists());
    assert!(!root.join("events/New.yssbi-event").exists());
    assert_eq!(error.code(), "stale_project_lifecycle");
    assert!(error.incident_id().is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rename_command_preserves_recovery_required_code_and_emits_nothing() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-recovery-rename-command-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let old_path = GraphResourcePath::new("events/Old.yssbi-event").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        old_path.clone(),
        GraphResourceDocument::new("Old", GraphDocumentKind::Event),
    );
    crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
    crate::project::fixtures::write_graph(&project, root.to_string_lossy().as_ref(), &old_path)
        .unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
    state
        .project_recovery_marker()
        .mark("unwind rollback failed");
    let project_instance_id = state.project_instance_id();
    let mut events = Vec::new();

    let error = rename_graph_resource_with_emitter(
        &state,
        ProjectInstanceId::from_existing(project_instance_id.clone()),
        old_path.clone(),
        ResourceRevision::INITIAL,
        "New",
        1,
        OperationId::new(),
        |event| events.push(event),
    )
    .unwrap_err();

    assert_eq!(error.code(), "project_recovery_required");
    assert_eq!(
        error.details(),
        serde_json::json!({ "recoveryRequired": true }).as_object()
    );
    assert!(events.is_empty());
    assert!(root.join(old_path.as_str()).exists());
    assert!(!root.join("events/New.yssbi-event").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn resource_command_emitter_failure_preserves_committed_receipt_observability() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-resource-command-emitter-failure-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let project_id = state.capture_project_session().unwrap().instance_id;
    let operation_id = OperationId::new();

    let result = create_graph_resource_with_emitter(
        &state,
        project_id.clone(),
        "Committed",
        GraphDocumentKind::Event,
        operation_id,
        |_| Err::<(), _>("emitter offline"),
    )
    .unwrap();

    assert_eq!(result.operation_id, operation_id);
    assert_eq!(result.project_instance_id, project_id.as_str());
    assert!(root.join("events/Committed.yssbi-event").is_file());
    let replay = state
        .create_graph_resource_transaction(
            &project_id,
            "Committed",
            GraphDocumentKind::Event,
            operation_id,
        )
        .unwrap_err();
    assert_eq!(replay.code(), "duplicate_operation");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn resource_commands_emit_one_project_scoped_committed_result() {
    let create_root = std::env::temp_dir().join(format!(
        "yssbi-resource-command-create-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&create_root).unwrap();
    crate::project::fixtures::write_project(
        &ProjectData::new(),
        create_root.to_string_lossy().as_ref(),
    )
    .unwrap();
    let create_state = ProjectState::new();
    create_state.activate_project_fixture(
        create_root.to_string_lossy().into_owned(),
        ProjectData::new(),
    );
    let create_id = create_state.capture_project_session().unwrap().instance_id;
    let mut create_events = Vec::new();
    let create_operation_id = OperationId::new();
    let created = create_graph_resource_with_emitter(
        &create_state,
        create_id.clone(),
        "Created",
        GraphDocumentKind::Event,
        create_operation_id,
        |event| create_events.push(event),
    )
    .unwrap();
    assert_eq!(created.operation_id, create_operation_id);
    assert_eq!(created.project_instance_id, create_id.as_str());
    assert_eq!(created.deltas.len(), 1);
    assert_eq!(
        created.deltas[0].resource,
        ResourceKey::Graph(
            crate::graph_document::GraphResourcePath::new("events/Created.yssbi-event").unwrap()
        )
    );
    assert_eq!(created.deltas[0].from_revision, ResourceRevision::INITIAL);
    assert_eq!(created.deltas[0].to_revision, ResourceRevision::INITIAL);
    assert_eq!(created.deltas[0].caused_by, Some(create_operation_id));
    assert_eq!(
        serde_json::to_value(&created.deltas[0].payload).unwrap(),
        serde_json::json!({
            "kind": "resource_lifecycle",
            "patch": {
                "before": null,
                "after": {
                    "revision": 0,
                    "path": "events/Created.yssbi-event",
                    "kind": "event",
                    "name": "Created"
                }
            }
        })
    );
    assert!(matches!(
        create_events.as_slice(),
        [Event::Project(EventProject::ResourceMutationCommitted { result })]
            if result == &created
    ));

    for operation in ["duplicate", "remove", "rename"] {
        let root = std::env::temp_dir().join(format!(
            "yssbi-resource-command-{operation}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = GraphResourcePath::new("events/Source.yssbi-event").unwrap();
        let mut data = ProjectData::new();
        data.graphs.insert(
            path.clone(),
            GraphResourceDocument::new("Source", GraphDocumentKind::Event),
        );
        crate::project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        crate::project::fixtures::write_graph(&data, root.to_string_lossy().as_ref(), &path)
            .unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
        let project_id = state.capture_project_session().unwrap().instance_id;
        let mut events = Vec::new();
        let operation_id = OperationId::new();
        let result = match operation {
            "duplicate" => duplicate_graph_resource_with_emitter(
                &state,
                project_id.clone(),
                path,
                ResourceRevision::INITIAL,
                operation_id,
                |event| events.push(event),
            ),
            "remove" => remove_graph_resource_with_emitter(
                &state,
                project_id.clone(),
                path,
                ResourceRevision::INITIAL,
                operation_id,
                |event| events.push(event),
            ),
            "rename" => rename_graph_resource_with_emitter(
                &state,
                project_id.clone(),
                path,
                ResourceRevision::INITIAL,
                "Renamed",
                1,
                operation_id,
                |event| events.push(event),
            ),
            _ => unreachable!(),
        }
        .unwrap();
        assert_eq!(result.operation_id, operation_id);
        assert_eq!(result.project_instance_id, project_id.as_str());
        if operation != "rename" {
            assert_eq!(
                result.deltas.len(),
                1,
                "{operation} must emit one lifecycle delta"
            );
            let delta = &result.deltas[0];
            assert_eq!(delta.from_revision, ResourceRevision::INITIAL);
            assert_eq!(
                delta.to_revision,
                if operation == "duplicate" {
                    ResourceRevision::INITIAL
                } else {
                    ResourceRevision::new(1)
                }
            );
            assert_eq!(delta.caused_by, Some(operation_id));
            let (expected_path, expected_name) = if operation == "remove" {
                ("events/Source.yssbi-event", "Source")
            } else {
                ("events/Source 2.yssbi-event", "Source 2")
            };
            assert_eq!(
                delta.resource,
                ResourceKey::Graph(
                    crate::graph_document::GraphResourcePath::new(expected_path).unwrap()
                )
            );
            let state = serde_json::json!({
                "revision": 0,
                "path": expected_path,
                "kind": "event",
                "name": expected_name
            });
            let (before, after) = if operation == "remove" {
                (state, serde_json::Value::Null)
            } else {
                (serde_json::Value::Null, state)
            };
            assert_eq!(
                serde_json::to_value(&delta.payload).unwrap(),
                serde_json::json!({
                    "kind": "resource_lifecycle",
                    "patch": { "before": before, "after": after }
                })
            );
        }
        assert!(matches!(
            events.as_slice(),
            [Event::Project(EventProject::ResourceMutationCommitted { result: emitted })]
                if emitted == &result
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
    std::fs::remove_dir_all(create_root).unwrap();
}

#[test]
fn rename_command_returns_and_emits_canonical_mutation_result() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-rename-command-event-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let old_path = GraphResourcePath::new("events/Old.yssbi-event").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        old_path.clone(),
        GraphResourceDocument::new("Old", GraphDocumentKind::Event),
    );
    crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
    let mut events = Vec::new();
    let project_instance_id = state.project_instance_id();

    let result = rename_graph_resource_with_emitter(
        &state,
        ProjectInstanceId::from_existing(project_instance_id.clone()),
        old_path.clone(),
        ResourceRevision::INITIAL,
        "New",
        1,
        OperationId::new(),
        |event| events.push(event),
    )
    .unwrap();

    assert_eq!(result.project_instance_id, project_instance_id);
    assert_eq!(result.publication_revision, 1);
    assert_eq!(result.moves.len(), 1);
    assert_eq!(result.moves[0].from, old_path.as_str());
    assert_eq!(result.moves[0].to, "events/New.yssbi-event");
    assert_eq!(result.deltas.len(), 1);
    assert_eq!(
        result.deltas[0].resource,
        ResourceKey::Graph(
            crate::graph_document::GraphResourcePath::new("events/New.yssbi-event").unwrap()
        )
    );
    assert_eq!(result.deltas[0].from_revision, ResourceRevision::INITIAL);
    assert_eq!(result.deltas[0].to_revision, ResourceRevision::new(1));
    assert!(result.deltas[0].caused_by.is_some());
    assert_eq!(result.projection_replacements.len(), 1);
    assert_eq!(
        result.projection_replacements[0].graph_path.as_str(),
        "events/New.yssbi-event"
    );
    assert_eq!(
        result.projection_replacements[0].projection.source_revision,
        1
    );
    assert_eq!(
        result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec!["events/New.yssbi-event".into()],
        }
    );
    assert!(result.history.can_undo);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        Event::Project(EventProject::ResourceMutationCommitted { result: emitted })
            if emitted == &result
    ));
    let _ = std::fs::remove_dir_all(root);
}
